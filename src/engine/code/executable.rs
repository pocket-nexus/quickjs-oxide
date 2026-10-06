//! Immutable execution projection built from authenticated published bytecode.
//! External snapshots own a bytecode root; internal snapshots borrow liveness
//! from their callee owner. Deref exposes shared facts, never mutable metadata.
use crate::engine::api::runtime::Runtime;
use crate::engine::api::runtime_error::RuntimeError;
use crate::engine::atom::Atom;
use crate::engine::code::function::metadata::{
    ClosureVariable, EvalEnvironment, FunctionMetadata, VariableDefinition,
};
use crate::engine::code::rooted::FunctionBytecodeRef;
use crate::engine::heap::runtime::RuntimeState;
use crate::engine::heap::{BytecodeConstant, ContextId, FunctionBytecodeData, FunctionBytecodeId};
use std::rc::Rc;

/// A rooted, immutable eval descriptor selected from its publisher's array.
/// Duplicating this view retains its bytecode root and shares the array; it
/// never copies scopes or bindings.
pub(crate) struct PublishedEvalEnvironment {
    owner: FunctionBytecodeRef,
    environments: Rc<[EvalEnvironment<Atom>]>,
    index: usize,
}

impl PublishedEvalEnvironment {
    pub(crate) fn try_clone(&self) -> Result<Self, RuntimeError> {
        Ok(Self {
            owner: self.owner.try_clone()?,
            environments: self.environments.clone(),
            index: self.index,
        })
    }

    pub(crate) fn same_environment(&self, other: &Self) -> bool {
        self.index == other.index && Rc::ptr_eq(&self.environments, &other.environments)
    }

    pub(crate) fn owner(&self) -> &FunctionBytecodeRef {
        &self.owner
    }
}

impl std::ops::Deref for PublishedEvalEnvironment {
    type Target = EvalEnvironment<Atom>;

    fn deref(&self) -> &Self::Target {
        &self.environments[self.index]
    }
}

/// Heap-resident certificate contains only immutable publication facts, never
/// Runtime or an external root. The function payload owns the bytecode edge.
#[derive(Debug, Clone)]
pub(crate) struct OrdinaryAuthentication {
    pub(crate) publish_generation: u64,
    pub(crate) closure_count: usize,
    pub(crate) data: Rc<PublishedFunctionData>,
}

impl PartialEq for OrdinaryAuthentication {
    fn eq(&self, other: &Self) -> bool {
        self.publish_generation == other.publish_generation
            && self.closure_count == other.closure_count
            && Rc::ptr_eq(&self.data, &other.data)
    }
}

pub(crate) struct PublishedFunctionSnapshot {
    root: std::cell::OnceCell<FunctionBytecodeRef>,
    bytecode: Option<FunctionBytecodeId>,
    runtime_domain: u64,
    data: Rc<PublishedFunctionData>,
}

impl std::ops::Deref for PublishedFunctionSnapshot {
    type Target = PublishedFunctionData;
    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl PublishedFunctionSnapshot {
    /// Borrow every static binding classification from this rooted owner.
    pub(crate) fn frame_layout(&self) -> crate::engine::code::function::layout::FrameLayout<'_> {
        #[cfg(test)]
        let plain_local_initializers = if self.bytecode.is_none() {
            // Synthetic snapshots are mutable in unit tests. Derive this fact
            // from their current definitions rather than a stale test default.
            self.metadata.function_name_local.is_none()
                && self.local_definitions.iter().all(|local| !local.is_lexical)
        } else {
            self.data.plain_local_initializers
        };
        #[cfg(not(test))]
        let plain_local_initializers = self.data.plain_local_initializers;
        crate::engine::code::function::layout::FrameLayout::new(
            &self.metadata,
            &self.argument_definitions,
            &self.local_definitions,
            &self.closure_variables,
            plain_local_initializers,
        )
    }

    /// One checked projection for all constant consumers. The opcode still
    /// chooses the kind-specific operation; this view owns no extra roots.
    #[inline]
    pub(crate) fn constant(&self, index: u32) -> Option<&BytecodeConstant> {
        usize::try_from(index)
            .ok()
            .and_then(|index| self.constants.get(index))
    }

    pub(crate) fn eval_environment(
        &self,
        index: u16,
    ) -> Result<Option<PublishedEvalEnvironment>, RuntimeError> {
        let index = usize::from(index);
        let Some(_) = self.eval_environments.get(index) else {
            return Ok(None);
        };
        let Some(owner) = self.root.get() else {
            return Ok(None);
        };
        Ok(Some(PublishedEvalEnvironment {
            owner: owner.try_clone()?,
            environments: self.eval_environments.clone(),
            index,
        }))
    }

    pub(crate) fn root(&self) -> Option<&FunctionBytecodeRef> {
        self.root.get()
    }

    /// The domain token rejects foreign published facts without owning Runtime.
    /// A rooted snapshot owns its bytecode; internal access instead requires
    /// the caller's live callee owner to keep that bytecode published.
    pub(crate) fn belongs_to(&self, runtime: &Runtime) -> bool {
        self.belongs_to_domain(runtime.domain_id())
    }

    pub(crate) fn belongs_to_domain(&self, domain: u64) -> bool {
        self.runtime_domain == domain
    }

    pub(crate) fn bytecode_id(&self) -> Option<FunctionBytecodeId> {
        self.bytecode
    }

    /// Cold observation boundary. Ordinary frames are already kept alive by
    /// their callee owner and therefore do not acquire this independent root
    /// until eval, suspension, or host materialization actually needs one.
    pub(crate) fn ensure_root(&self, runtime: &Runtime) -> Result<(), RuntimeError> {
        if self.bytecode.is_some() && !self.belongs_to(runtime) {
            return Err(RuntimeError::WrongRuntime("function bytecode"));
        }
        if self.root.get().is_none() {
            if let Some(id) = self.bytecode {
                let root = FunctionBytecodeRef::from_borrowed_handle(runtime.clone(), id)?;
                let _ = self.root.set(root);
            }
        }
        Ok(())
    }

    /// The caller pairs the admitted state with its runtime domain and holds
    /// the callee owning this bytecode. Generation and closure facts must have
    /// been checked against that same live function before construction.
    pub(crate) fn from_authentication_in_domain(
        domain_id: u64,
        id: FunctionBytecodeId,
        facts: OrdinaryAuthentication,
    ) -> Self {
        Self {
            root: Default::default(),
            bytecode: Some(id),
            runtime_domain: domain_id,
            data: facts.data,
        }
    }

    #[cfg(test)]
    pub(crate) fn empty_for_test(realm: ContextId) -> Self {
        Self {
            root: Default::default(),
            bytecode: None,
            runtime_domain: 0,
            data: Rc::new(PublishedFunctionData {
                has_captured_locals: true,
                observes_arguments: true,
                plain_local_initializers: true,

                property_read_ic:
                    crate::engine::object::property_ic::PropertyReadCacheTable::new_exec(
                        &crate::engine::code::exec::ExecCode::empty(),
                    ),
                property_append_ic:
                    crate::engine::object::append_ic::PropertyAppendCacheTable::new_exec(
                        &crate::engine::code::exec::ExecCode::empty(),
                    ),
                exec: crate::engine::code::exec::ExecCode::empty(),
                constants: Rc::from([]),
                property_key_atoms: None,
                argument_definitions: Rc::from([]),
                local_definitions: Rc::from([]),
                closure_variables: Rc::from([]),
                eval_environments: Rc::from([]),
                arg_eval_variable_object_local: None,
                metadata: FunctionMetadata::default(),
                realm,
            }),
        }
    }
}

// Synthetic host fixtures exercise rejected internal operations. They never
// provide a production constructor or a mutable view in non-test builds.
#[cfg(test)]
impl std::ops::DerefMut for PublishedFunctionSnapshot {
    fn deref_mut(&mut self) -> &mut Self::Target {
        assert!(
            self.bytecode.is_none(),
            "published snapshots remain immutable in tests"
        );
        Rc::get_mut(&mut self.data).expect("synthetic executable remains uniquely owned")
    }
}

#[derive(Debug)]
pub(crate) struct PublishedFunctionData {
    pub(crate) has_captured_locals: bool,
    pub(crate) observes_arguments: bool,
    pub(crate) plain_local_initializers: bool,

    pub(crate) property_read_ic: crate::engine::object::property_ic::PropertyReadCacheTable,
    pub(crate) property_append_ic: crate::engine::object::append_ic::PropertyAppendCacheTable,
    pub(crate) exec: crate::engine::code::exec::ExecCode,
    pub(crate) constants: Rc<[BytecodeConstant]>,
    pub(crate) property_key_atoms: Option<Rc<[Atom]>>,
    pub(crate) argument_definitions: Rc<[VariableDefinition]>,
    pub(crate) local_definitions: Rc<[VariableDefinition]>,
    pub(crate) closure_variables: Rc<[ClosureVariable]>,
    pub(crate) eval_environments: Rc<[EvalEnvironment<Atom>]>,
    /// Parameter-scope variable-object slot, carried separately from the
    /// body `<var>` slot in `FunctionMetadata`.
    pub(crate) arg_eval_variable_object_local: Option<u16>,
    pub(crate) metadata: FunctionMetadata,
    pub(crate) realm: ContextId,
}

impl Runtime {
    pub(crate) fn snapshot_function_bytecode(
        &self,
        function: &FunctionBytecodeRef,
    ) -> Result<PublishedFunctionSnapshot, RuntimeError> {
        if !function.belongs_to(self) {
            return Err(RuntimeError::WrongRuntime("function bytecode"));
        }
        self.snapshot_function_bytecode_owned(function.try_clone()?)
    }

    pub(crate) fn snapshot_function_bytecode_owned(
        &self,
        function: FunctionBytecodeRef,
    ) -> Result<PublishedFunctionSnapshot, RuntimeError> {
        let _operation = self.operation()?;
        if !function.belongs_to(self) {
            return Err(RuntimeError::WrongRuntime("function bytecode"));
        }
        let state = self.0.state.borrow();
        let data = state.published_function_data(function.bytecode_id())?;

        Ok(PublishedFunctionSnapshot {
            runtime_domain: self.domain_id(),
            bytecode: Some(function.bytecode_id()),
            root: std::cell::OnceCell::from(function),
            data,
        })
    }
}

impl RuntimeState {
    fn published_function_data(
        &self,
        id: FunctionBytecodeId,
    ) -> Result<Rc<PublishedFunctionData>, RuntimeError> {
        let bytecode = self.heap.function_bytecode(id)?;
        // The realm is a strong bytecode edge, checked before entering a frame.
        self.heap.context(bytecode.realm)?;
        Ok(published_function_data(bytecode))
    }

    /// Authenticate the cold ordinary-call path without an independent root.
    /// The caller already owns the function's bytecode edge; a raw ID itself
    /// is not a liveness proof. Weak cached certificates must still be checked
    /// for publication generation and closure count by their actual consumer.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn authenticate_ordinary_bytecode(
        &self,
        id: FunctionBytecodeId,
        closure_count: usize,
    ) -> Result<Option<OrdinaryAuthentication>, RuntimeError> {
        let bytecode = self.heap.function_bytecode(id)?;
        if bytecode.metadata.function_kind
            != crate::engine::code::function::metadata::FunctionKind::Normal
        {
            return Ok(None);
        }
        if closure_count != usize::from(bytecode.metadata.closure_count) {
            return Err(RuntimeError::Invariant(
                "function object closure slot count does not match bytecode metadata",
            ));
        }
        self.heap.context(bytecode.realm)?;
        Ok(Some(OrdinaryAuthentication {
            publish_generation: id.publish_generation(),
            closure_count,
            data: published_function_data(bytecode),
        }))
    }
}

// Both external snapshots and internal authentication share the immutable
// projection resident on this publication. Building it owns no heap edge.
fn published_function_data(bytecode: &FunctionBytecodeData) -> Rc<PublishedFunctionData> {
    let data =
        bytecode.executable.get_or_init(|| {
            let data = Rc::new(PublishedFunctionData {
                has_captured_locals: !bytecode.local_definitions.is_empty()
                    && (0..bytecode.exec.instruction_len()).any(|pc| {
                        matches!(
                            bytecode.exec.opcode_at_source(pc),
                            Some(
                                crate::engine::code::exec_opcode::Opcode::FClosure
                                    | crate::engine::code::exec_opcode::Opcode::Eval
                                    | crate::engine::code::exec_opcode::Opcode::ApplyEval
                            )
                        )
                    }),
                observes_arguments: (0..bytecode.exec.instruction_len()).any(|pc| {
                    matches!(
                        bytecode.exec.opcode_at_source(pc),
                        Some(
                            crate::engine::code::exec_opcode::Opcode::Arguments
                                | crate::engine::code::exec_opcode::Opcode::Rest
                                | crate::engine::code::exec_opcode::Opcode::Eval
                                | crate::engine::code::exec_opcode::Opcode::ApplyEval
                        )
                    )
                }),
                plain_local_initializers: bytecode.metadata.function_name_local.is_none()
                    && bytecode
                        .local_definitions
                        .iter()
                        .all(|local| !local.is_lexical),

                property_read_ic:
                    crate::engine::object::property_ic::PropertyReadCacheTable::new_exec(
                        &bytecode.exec,
                    ),
                property_append_ic:
                    crate::engine::object::append_ic::PropertyAppendCacheTable::new_exec(
                        &bytecode.exec,
                    ),
                exec: bytecode.exec.clone(),
                constants: bytecode.constants.clone(),
                property_key_atoms: bytecode.property_key_atoms.clone(),
                argument_definitions: bytecode.argument_definitions.clone(),
                local_definitions: bytecode.local_definitions.clone(),
                closure_variables: bytecode.closure_variables.clone(),
                eval_environments: bytecode.eval_environments.clone(),
                arg_eval_variable_object_local: bytecode
                    .parameter_environment
                    .as_ref()
                    .and_then(|layout| layout.arg_eval_variable_object_local),
                metadata: bytecode.metadata,
                realm: bytecode.realm,
            });
            #[cfg(feature = "profiling")]
            crate::engine::api::profiling::record_call_buffer_capacity(
                "executable.published_data_rc",
                0,
                1,
                size_of::<PublishedFunctionData>(),
            );
            data
        });
    let data = data.clone();
    #[cfg(feature = "profiling")]
    crate::engine::api::profiling::record_call_buffer_share(
        "executable.published_data_rc",
        1,
        size_of::<PublishedFunctionData>(),
    );
    data
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::code::bytecode::Instruction;
    use crate::engine::code::function::{UnlinkedFunction, UnlinkedVariableDefinition};

    fn publish(runtime: &Runtime, realm: ContextId) -> FunctionBytecodeRef {
        runtime
            .publish_unlinked_function(
                realm,
                UnlinkedFunction::fixture(
                    vec![Instruction::PushI32(42), Instruction::Return],
                    vec![],
                    FunctionMetadata {
                        max_stack: 1,
                        ..FunctionMetadata::default()
                    },
                ),
            )
            .unwrap()
    }

    #[test]
    fn direct_state_authentication_preserves_shared_publication_without_roots() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        let crate::engine::value::Value::Object(function) = context
            .eval(
                "(function(a){let lexical=a.x;function inner(){return lexical};return arguments})",
            )
            .unwrap()
        else {
            panic!("function")
        };
        let state = runtime.0.state.borrow_mut();
        let crate::engine::heap::ObjectPayload::BytecodeFunction {
            bytecode,
            closure_slots,
            ..
        } = &state.heap.object(function.object_id()).unwrap().payload
        else {
            panic!("bytecode function")
        };
        let id = *bytecode;
        let count = closure_slots.len();
        let runtime_owners = Rc::strong_count(&runtime.0);
        let bytecode_owners = state.heap.function_bytecode_strong_count(id).unwrap();
        let function_owners = state
            .heap
            .object_strong_count(function.object_id())
            .unwrap();
        let facts = state
            .authenticate_ordinary_bytecode(id, count)
            .unwrap()
            .unwrap();
        let shared = state
            .authenticate_ordinary_bytecode(id, count)
            .unwrap()
            .unwrap();
        assert_eq!(facts.publish_generation, id.publish_generation());
        assert_eq!(facts.closure_count, count);
        assert!(Rc::ptr_eq(&facts.data, &shared.data));
        let snapshot = PublishedFunctionSnapshot::from_authentication_in_domain(
            runtime.domain_id(),
            id,
            facts,
        );
        assert!(snapshot.root().is_none());
        assert!(snapshot.belongs_to_domain(runtime.domain_id()));
        assert!(!snapshot.belongs_to_domain(runtime.domain_id() + 1));
        assert!(snapshot.has_captured_locals);
        assert!(snapshot.observes_arguments);
        assert!(!snapshot.frame_layout().plain_local_initializers());
        let published = state.heap.function_bytecode(id).unwrap();
        assert!(Rc::ptr_eq(&snapshot.constants, &published.constants));
        assert!(Rc::ptr_eq(
            &snapshot.argument_definitions,
            &published.argument_definitions
        ));
        assert!(Rc::ptr_eq(
            &snapshot.local_definitions,
            &published.local_definitions
        ));
        assert!(Rc::ptr_eq(
            &snapshot.closure_variables,
            &published.closure_variables
        ));
        assert!(Rc::ptr_eq(
            &snapshot.eval_environments,
            &published.eval_environments
        ));
        assert!(Rc::ptr_eq(
            snapshot.property_key_atoms.as_ref().unwrap(),
            published.property_key_atoms.as_ref().unwrap()
        ));
        assert!(Rc::ptr_eq(
            &snapshot.data,
            published.executable.get().unwrap()
        ));
        assert!(std::ptr::eq(
            snapshot.exec.test_ir(),
            published.exec.test_ir()
        ));
        assert_eq!(snapshot.metadata, published.metadata);
        #[cfg(feature = "profiling")]
        {
            assert_eq!(
                snapshot.exec.word_storage_identity(),
                published.exec.word_storage_identity()
            );
            assert_eq!(
                snapshot.exec.boundary_storage_identity(),
                published.exec.boundary_storage_identity()
            );
        }
        assert_eq!(Rc::strong_count(&runtime.0), runtime_owners);
        assert_eq!(
            state.heap.function_bytecode_strong_count(id).unwrap(),
            bytecode_owners
        );
        assert_eq!(
            state
                .heap
                .object_strong_count(function.object_id())
                .unwrap(),
            function_owners
        );
        assert!(!runtime.0.deferred_references.has_pending());
    }

    #[test]
    fn direct_state_authentication_rejects_bad_closures_before_creating_facts() {
        let runtime = Runtime::new();
        let context = runtime.new_context().expect("create context");
        let function = publish(&runtime, context.realm);
        let state = runtime.0.state.borrow_mut();
        let id = function.bytecode_id();
        assert!(
            state
                .heap
                .function_bytecode(id)
                .unwrap()
                .executable
                .get()
                .is_none()
        );
        let error = state.authenticate_ordinary_bytecode(id, 1).unwrap_err();
        assert!(matches!(
            error,
            RuntimeError::Invariant(
                "function object closure slot count does not match bytecode metadata"
            )
        ));
        assert!(
            state
                .heap
                .function_bytecode(id)
                .unwrap()
                .executable
                .get()
                .is_none()
        );
        assert_eq!(state.heap.function_bytecode_strong_count(id).unwrap(), 1);
    }

    #[test]
    fn direct_state_authentication_declines_nonordinary_function_kinds() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        for source in ["(async function(){return 42})", "(function*(){return 42})"] {
            let crate::engine::value::Value::Object(function) = context.eval(source).unwrap()
            else {
                panic!("function")
            };
            let state = runtime.0.state.borrow_mut();
            let crate::engine::heap::ObjectPayload::BytecodeFunction {
                bytecode,
                closure_slots,
                ..
            } = &state.heap.object(function.object_id()).unwrap().payload
            else {
                panic!("bytecode function")
            };
            assert!(
                state
                    .authenticate_ordinary_bytecode(*bytecode, closure_slots.len())
                    .unwrap()
                    .is_none()
            );
            assert!(
                state
                    .heap
                    .function_bytecode(*bytecode)
                    .unwrap()
                    .executable
                    .get()
                    .is_none()
            );
        }
    }

    #[test]
    fn direct_state_authentication_rejects_stale_publication_after_arena_reuse() {
        let runtime = Runtime::new();
        let context = runtime.new_context().expect("create context");
        let function = publish(&runtime, context.realm);
        let old_id = function.bytecode_id();
        let old = runtime
            .0
            .state
            .borrow_mut()
            .authenticate_ordinary_bytecode(old_id, 0)
            .unwrap()
            .unwrap();
        drop(function);
        let replacement = publish(&runtime, context.realm);
        let state = runtime.0.state.borrow_mut();
        assert!(state.authenticate_ordinary_bytecode(old_id, 0).is_err());
        let fresh = state
            .authenticate_ordinary_bytecode(replacement.bytecode_id(), 0)
            .unwrap()
            .unwrap();
        assert_eq!(fresh.publish_generation >> 32, old.publish_generation >> 32);
        assert_ne!(fresh.publish_generation, old.publish_generation);
        assert!(!Rc::ptr_eq(&fresh.data, &old.data));
    }

    #[test]
    fn snapshot_retains_its_owner_and_rejects_another_runtime() {
        let runtime = Runtime::new();
        let context = runtime.new_context().expect("create context");
        let function = publish(&runtime, context.realm);
        let other = Runtime::new();
        assert!(matches!(
            other.snapshot_function_bytecode(&function),
            Err(RuntimeError::WrongRuntime("function bytecode"))
        ));
        let snapshot = runtime.snapshot_function_bytecode(&function).unwrap();
        let second = runtime.snapshot_function_bytecode(&function).unwrap();
        assert!(Rc::ptr_eq(&snapshot.data, &second.data));
        // Two frame headers share one projection but each keeps the bytecode
        // root alive independently. The cache itself owns no rooting handle.
        drop(second);
        let id = function.bytecode_id();
        drop(function);
        assert_eq!(snapshot.root().unwrap().bytecode_id(), id);
        assert!(matches!(
            snapshot.exec.test_ir(),
            [Instruction::PushI32(42), Instruction::Return]
        ));
        assert!(runtime.0.state.borrow().heap.function_bytecode(id).is_ok());
    }

    #[test]
    fn published_local_initializer_fact_is_shared_and_rejects_lexical_bindings() {
        let runtime = Runtime::new();
        let context = runtime.new_context().expect("create context");
        let metadata = FunctionMetadata {
            local_count: 2,
            max_stack: 1,
            ..FunctionMetadata::default()
        };
        let plain = runtime
            .publish_unlinked_function(
                context.realm,
                UnlinkedFunction::fixture(
                    vec![Instruction::PushI32(42), Instruction::Return],
                    vec![],
                    metadata,
                ),
            )
            .unwrap();
        let plain_snapshot = runtime.snapshot_function_bytecode(&plain).unwrap();
        let shared = runtime.snapshot_function_bytecode(&plain).unwrap();
        assert!(Rc::ptr_eq(&plain_snapshot.data, &shared.data));
        assert!(plain_snapshot.frame_layout().plain_local_initializers());
        assert!(shared.frame_layout().plain_local_initializers());

        let lexical = runtime
            .publish_unlinked_function(
                context.realm,
                UnlinkedFunction::fixture(
                    vec![Instruction::PushI32(42), Instruction::Return],
                    vec![],
                    metadata,
                )
                .with_fixture_definitions(
                    vec![],
                    vec![
                        UnlinkedVariableDefinition::ordinary(None),
                        UnlinkedVariableDefinition::lexical(None, false),
                    ],
                ),
            )
            .unwrap();
        assert!(
            !runtime
                .snapshot_function_bytecode(&lexical)
                .unwrap()
                .frame_layout()
                .plain_local_initializers()
        );
    }

    #[test]
    fn eval_view_shares_storage_but_authenticates_the_selected_environment() {
        use crate::engine::code::function::metadata::EvalVariableEnvironment;
        let runtime = Runtime::new();
        let context = runtime.new_context().expect("create context");
        let owner = publish(&runtime, context.realm);
        let id = owner.bytecode_id();
        let environment = EvalEnvironment {
            scopes: Box::new([]),
            variable_environment: EvalVariableEnvironment::Global,
            caller_strict: false,
            super_call_allowed: false,
            super_allowed: false,
        };
        let view = PublishedEvalEnvironment {
            owner,
            environments: Rc::from([environment.clone(), environment.clone()]),
            index: 0,
        };
        let shared = view.try_clone().expect("duplicate environment");
        assert!(view.same_environment(&shared));
        let mut another_index = view.try_clone().expect("duplicate environment");
        another_index.index = 1;
        assert!(!view.same_environment(&another_index));
        let mut another_owner = view.try_clone().expect("duplicate environment");
        another_owner.environments = Rc::from([environment]);
        assert!(!view.same_environment(&another_owner));
        drop(view);
        drop(another_index);
        drop(another_owner);
        assert!(runtime.0.state.borrow().heap.function_bytecode(id).is_ok());
        assert!(!shared.caller_strict);
        drop(shared);
        assert!(runtime.0.state.borrow().heap.function_bytecode(id).is_err());
    }

    #[test]
    #[should_panic(expected = "published snapshots remain immutable in tests")]
    fn synthetic_fixture_mutation_cannot_change_published_code() {
        let runtime = Runtime::new();
        let context = runtime.new_context().expect("create context");
        let function = publish(&runtime, context.realm);
        let mut snapshot = runtime.snapshot_function_bytecode(&function).unwrap();
        snapshot.exec = crate::engine::code::exec::ExecCode::empty();
    }
}
