//! Shared suspension ownership for generators, async functions and async generators.
//!
//! Frozen raw records borrow their source roots until heap publication completes.
//! Thaw reconstructs all roots before language owners detach dormant heap edges.
//! The language state machines and microtask policy stay with their own drivers.

use crate::engine::api::{runtime::Runtime, runtime_error::RuntimeError};
use crate::engine::atom::{Atom, AtomKind};
use crate::engine::code::function::metadata::{
    ClosureVariableKind, FunctionKind, VariableDefinition,
};
use crate::engine::code::rooted::FunctionBytecodeRef;
use crate::engine::code::runtime::PublishedFunctionData;
use crate::engine::heap::ownership::ConvertedValue;
use crate::engine::heap::{
    ContextId, GeneratorActivationData, GeneratorFrameBinding, GeneratorVmActivation, RawValue,
};
use crate::engine::object::ObjectRef;
use crate::engine::value::JsValue;
use crate::engine::vm::bindings::{FrameBinding, is_private_callable_kind, release_frame_binding};
use crate::engine::vm::call::CallableExecution;
use crate::engine::vm::frames::ActiveFrameToken;
use crate::engine::vm::{BytecodePc, Completion, VmResume, VmSuspendKind};

pub(super) mod creation;

mod owned;

pub(super) use owned::{OwnedSuspension, PreparedResume};

/// Reconstruct one owned internal value from a dormant heap record, retaining
/// every edge so the decoded value owns them independently of the record.
fn decode_raw_jsvalue(runtime: &Runtime, raw: RawValue) -> Result<JsValue, RuntimeError> {
    let value = JsValue::from_raw(raw).ok_or(RuntimeError::Invariant(
        "dormant activation held an internal-only sentinel",
    ))?;
    runtime.dup_jsvalue(&value)
}

fn encode_generator_frame_binding(binding: &FrameBinding) -> GeneratorFrameBinding {
    match binding {
        FrameBinding::Direct(value) => GeneratorFrameBinding::Direct(value.as_raw()),
        FrameBinding::Private(index) => GeneratorFrameBinding::Private(*index),
        FrameBinding::PrivateCallable(object) => GeneratorFrameBinding::PrivateCallable(*object),
        FrameBinding::Uninitialized => GeneratorFrameBinding::Uninitialized,
        FrameBinding::Captured(var_ref) => GeneratorFrameBinding::Captured(*var_ref),
    }
}

fn validate_decoded_generator_binding(
    runtime: &Runtime,
    binding: &FrameBinding,
    definition: Option<&VariableDefinition>,
) -> Result<(), RuntimeError> {
    let Some(definition) = definition else {
        if matches!(binding, FrameBinding::Direct(_)) {
            return Ok(());
        }
        return Err(RuntimeError::Invariant(
            "extra generator argument has a non-direct binding",
        ));
    };
    match binding {
        FrameBinding::Direct(_) if definition.kind.is_private() => Err(RuntimeError::Invariant(
            "generator private definition decoded as an ordinary value",
        )),
        FrameBinding::Private(_)
            if definition.kind != ClosureVariableKind::PrivateField
                || !definition.is_lexical
                || !definition.is_const =>
        {
            Err(RuntimeError::Invariant(
                "generator private-name binding disagrees with its definition",
            ))
        }
        FrameBinding::PrivateCallable(_)
            if !is_private_callable_kind(definition.kind)
                || !definition.is_lexical
                || !definition.is_const =>
        {
            Err(RuntimeError::Invariant(
                "generator private-callable binding disagrees with its definition",
            ))
        }
        FrameBinding::Uninitialized if !definition.is_lexical => Err(RuntimeError::Invariant(
            "generator non-lexical binding decoded as uninitialized",
        )),
        FrameBinding::Captured(var_ref) => {
            let state = runtime.0.state.borrow();
            let cell = state.heap.var_ref(*var_ref)?;
            if (cell.is_lexical, cell.is_const, cell.kind)
                != (definition.is_lexical, definition.is_const, definition.kind)
            {
                return Err(RuntimeError::Invariant(
                    "generator captured binding metadata disagrees with its definition",
                ));
            }
            Ok(())
        }
        FrameBinding::Direct(_)
        | FrameBinding::Private(_)
        | FrameBinding::PrivateCallable(_)
        | FrameBinding::Uninitialized => Ok(()),
    }
}

fn decode_generator_frame_binding(
    runtime: &Runtime,
    binding: &GeneratorFrameBinding,
    definition: Option<&VariableDefinition>,
) -> Result<FrameBinding, RuntimeError> {
    let binding = match binding {
        GeneratorFrameBinding::Direct(value) => {
            FrameBinding::Direct(decode_raw_jsvalue(runtime, value.clone())?)
        }
        GeneratorFrameBinding::Private(index) => {
            let atom = runtime.0.state.borrow().atoms.brand(*index)?;
            if runtime.0.state.borrow().atoms.kind(atom)? != AtomKind::Private {
                return Err(RuntimeError::Invariant(
                    "generator private binding contains a non-private atom",
                ));
            }
            runtime.retain_atom_handle(atom)?;
            FrameBinding::Private(*index)
        }
        GeneratorFrameBinding::PrivateCallable(object) => {
            // Validate callability through a temporary root, then retain the
            // frame's own object edge.
            let object_root = ObjectRef::from_borrowed_handle(runtime.clone(), *object)?;
            let callable = runtime
                .as_callable(&object_root)?
                .ok_or(RuntimeError::Invariant(
                    "generator private callable binding lost callability",
                ))?;
            drop(callable);
            drop(object_root);
            runtime.retain_object_handle(*object)?;
            FrameBinding::PrivateCallable(*object)
        }
        GeneratorFrameBinding::Uninitialized => FrameBinding::Uninitialized,
        GeneratorFrameBinding::Captured(var_ref) => {
            runtime.retain_var_ref_handle(*var_ref)?;
            FrameBinding::Captured(*var_ref)
        }
    };
    if let Err(error) = validate_decoded_generator_binding(runtime, &binding, definition) {
        // A rejected record must not keep the freshly retained edge. Handle
        // releases are nothrow; a Direct release failure is secondary to the
        // validation error on this invariant path.
        let _ = release_frame_binding(runtime, binding);
        return Err(error);
    }
    Ok(binding)
}

/// Raw resumable activation plus every transient root from which it was
/// encoded. The wrapper must outlive heap publication: raw
/// object/VarRef/bytecode/context identities are non-owning until a generator
/// object or hidden async-function state retains them.
pub(crate) struct EncodedVmActivation {
    pub(crate) kind: VmSuspendKind,
    pub(crate) data: GeneratorActivationData,
    _entry: EncodedActivationEntry,
}

impl EncodedVmActivation {
    /// Brand every retained atom index for the caller's explicit retain pass.
    /// Every binding stores unbranded indices; this boundary conversion is the
    /// only place a dormant activation's atoms become branded again.
    pub(crate) fn atoms(
        &self,
        table: &crate::engine::atom::AtomTable,
    ) -> Result<Vec<Atom>, RuntimeError> {
        let vm = &self.data.vm;
        let mut atoms = Vec::new();
        for value in vm
            .stack
            .iter()
            .chain(self.data.original_arguments.iter())
            .chain(std::iter::once(&vm.this_value))
            .chain(vm.normalized_this.iter())
            .chain(std::iter::once(&vm.new_target))
        {
            if let RawValue::Symbol(index) | RawValue::Private(index) = value {
                atoms.push(table.brand(*index)?);
            }
        }
        for binding in self.data.arguments.iter().chain(self.data.locals.iter()) {
            match binding {
                GeneratorFrameBinding::Direct(value) => {
                    if let RawValue::Symbol(index) | RawValue::Private(index) = value {
                        atoms.push(table.brand(*index)?);
                    }
                }
                GeneratorFrameBinding::Private(index) => atoms.push(table.brand(*index)?),
                GeneratorFrameBinding::PrivateCallable(_)
                | GeneratorFrameBinding::Uninitialized
                | GeneratorFrameBinding::Captured(_) => {}
            }
        }
        Ok(atoms)
    }

    /// Balance direct storage string/BigInt edges after publication. The
    /// matching storage cleanup skips these values. CallInput and FrameCold
    /// independently own this/newTarget/normalized-this, so their raw aliases
    /// must never be released here.
    pub(crate) fn release_conversion_edges(&mut self, runtime: &Runtime) {
        let vm = &self.data.vm;
        for value in vm.stack.iter().chain(self.data.original_arguments.iter()) {
            drop(ConvertedValue::new(runtime, value.clone()));
        }
        for binding in self.data.arguments.iter().chain(self.data.locals.iter()) {
            if let GeneratorFrameBinding::Direct(value) = binding {
                drop(ConvertedValue::new(runtime, value.clone()));
            }
        }
    }
}

/// Owns the source frame entry across heap publication and releases the
/// remaining caller-owned object/symbol/binding edges when the activation is
/// finally abandoned. Direct String/BigInt edges are the boundary-conversion
/// producer edges already released through `release_conversion_edges`, so the
/// storage release skips them. The drop runs after any state borrow has been
/// released, keeping releases nothrow.
struct EncodedActivationEntry {
    runtime: Runtime,
    entry: Option<super::frame::FrameEntry>,
}

impl Drop for EncodedActivationEntry {
    fn drop(&mut self) {
        let Some(mut entry) = self.entry.take() else {
            return;
        };
        let storage = std::mem::replace(
            &mut entry.storage,
            super::stack::FrameStorage {
                original_arguments: Vec::new(),
                parameters: Vec::new(),
                locals: Vec::new(),
                operands: Vec::new(),
            },
        );
        super::stack::release_unconverted_frame_storage(&self.runtime, storage);
    }
}

/// Fully rooted execution state reconstructed before its dormant heap edges
/// are detached. `host.active_frame_token` remains a sentinel until the
/// short-lived bytecode active frame is pushed for the actual resume.
pub(crate) struct RootedVmActivation {
    entry: Option<super::frame::FrameEntry>,
    kind: VmSuspendKind,
    saved_pc: usize,
}

impl Drop for RootedVmActivation {
    fn drop(&mut self) {
        let Some(entry) = self.entry.take() else {
            return;
        };
        let runtime = entry.cold.function.runtime().clone();
        super::stack::release_frame_storage(&runtime, entry.storage);
    }
}

pub(crate) enum VmActivationResume {
    Initial,
    Generator(VmResume),
    AwaitFulfill(JsValue),
    AwaitReject(JsValue),
}

pub(crate) enum VmRunOutcome {
    Complete(Completion),
    Suspend {
        value: JsValue,
        activation: Box<EncodedVmActivation>,
    },
}

impl RootedVmActivation {
    fn validate_resume(
        &self,
        runtime: &Runtime,
        resume: &VmActivationResume,
    ) -> Result<(), RuntimeError> {
        // Authenticate the resume input before installing any active frame or
        // invoking an unwinder. The dormant owner remains with the language
        // state machine until thaw has produced this single-use rooted value.
        let entry = self
            .entry
            .as_ref()
            .expect("rooted activation entry is present before prepare");
        if entry.cold.function.runtime().domain_id() != runtime.domain_id() {
            return Err(RuntimeError::WrongRuntime("suspended execution"));
        }
        // Internal resume values are handle-only and carry no runtime tag, so
        // their domain is guaranteed by the state machine that produced them;
        // only the activation's own runtime is authenticated here.
        match resume {
            VmActivationResume::Initial => {}
            VmActivationResume::Generator(_)
            | VmActivationResume::AwaitFulfill(_)
            | VmActivationResume::AwaitReject(_) => {}
        }
        Ok(())
    }

    pub(crate) fn run(
        self,
        runtime: &Runtime,
        resume: VmActivationResume,
    ) -> Result<VmRunOutcome, RuntimeError> {
        let prepared = self.prepare_owned(runtime, resume)?;
        super::driver::resume(runtime.clone(), prepared.entry, prepared.pc)
            .and_then(|exit| exit.finish_suspending(runtime.clone()))
            .map_err(RuntimeError::Engine)
    }

    pub(super) fn prepare_owned(
        mut self,
        runtime: &Runtime,
        resume: VmActivationResume,
    ) -> Result<PreparedResume, RuntimeError> {
        let mut pending_resume = Some(resume);
        let prepared = (|| {
            self.validate_resume(
                runtime,
                pending_resume.as_ref().expect("owned resume input"),
            )?;
            let entry = self
                .entry
                .as_mut()
                .expect("rooted activation entry is prepared once");
            let root = entry
                .executable
                .root()
                .ok_or(RuntimeError::Invariant(
                    "resumable frame has no published root",
                ))?
                .clone();
            let guard = runtime.push_bytecode_active_frame(
                (*entry.cold.function).clone(),
                root,
                entry.executable.realm,
                entry.executable.frame_layout().is_strict(),
            )?;
            entry.active_frame = guard.token();
            runtime.update_active_bytecode_pc(
                guard.token(),
                BytecodePc::new(self.saved_pc.saturating_sub(1)),
            )?;
            entry.cold.entry_guard = Some(guard);
            // Both frame storage and the resume input retain their original
            // owners through validation and the final fallible reservation.
            owned::prepare(entry, self.kind, &mut pending_resume)
        })();
        if let Err(error) = prepared {
            if let Some(resume) = pending_resume {
                match resume {
                    VmActivationResume::Initial => {}
                    VmActivationResume::Generator(
                        VmResume::Next(value) | VmResume::Return(value) | VmResume::Throw(value),
                    )
                    | VmActivationResume::AwaitFulfill(value)
                    | VmActivationResume::AwaitReject(value) => {
                        let _ = runtime.release_jsvalue(value);
                    }
                }
            }
            return Err(error);
        }
        Ok(PreparedResume {
            entry: self.entry.take().expect("prepared activation entry"),
            pc: self.saved_pc,
        })
    }
}

pub(super) fn freeze_entry(
    runtime: &Runtime,
    entry: super::frame::FrameEntry,
    kind: VmSuspendKind,
    pc: usize,
) -> Result<EncodedVmActivation, RuntimeError> {
    #[cfg(feature = "profiling")]
    let _profile_phase = crate::engine::api::profiling::PhaseTimer::start_vm("freeze.encode");
    // Encoding has not yet published or balanced any conversion edges. Keep
    // the ordinary frame owner until the complete encoded record exists.
    let mut source = RootedVmActivation {
        entry: Some(entry),
        kind,
        saved_pc: pc,
    };
    let entry = source.entry.as_mut().expect("owned encoding source");
    entry
        .cold
        .input
        .callee_global(runtime, entry.executable.realm)?;
    entry
        .cold
        .reusable_captured_locals
        .resize(entry.storage.locals.len(), false);
    let bytecode = entry.executable.root().ok_or(RuntimeError::Invariant(
        "resumable frame has no published root",
    ))?;
    let input = &*entry.cold.input;
    let global = input.callee_global.as_ref().ok_or(RuntimeError::Invariant(
        "resumable frame has no callee global",
    ))?;
    let storage = &entry.storage;
    let arguments = storage
        .parameters
        .iter()
        .map(encode_generator_frame_binding)
        .collect::<Vec<_>>();
    let locals = storage
        .locals
        .iter()
        .map(encode_generator_frame_binding)
        .collect::<Vec<_>>();
    let normalized_this = entry.cold.normalized_this.as_ref().map(JsValue::as_raw);
    let vm = GeneratorVmActivation {
        stack: storage
            .operands
            .iter()
            .map(|value| value.as_raw())
            .collect(),
        regions: entry.cold.regions.clone(),
        pc,
        callee_realm: entry.executable.realm,
        current_function: entry.cold.function.object_id(),
        this_value: input.this_value.as_raw(),
        normalized_this,
        new_target: input.new_target.as_raw(),
        strict: entry.executable.frame_layout().is_strict(),
        callee_global: global.object_id(),
    };
    let data = GeneratorActivationData {
        bytecode: bytecode.bytecode_id(),
        vm,
        actual_argument_count: storage.original_arguments.len(),
        original_arguments: storage
            .original_arguments
            .iter()
            .map(|value| value.as_raw())
            .collect(),
        arguments,
        locals,
        reusable_captured_locals: entry.cold.reusable_captured_locals.clone(),
    };
    Ok(EncodedVmActivation {
        kind,
        data,
        _entry: EncodedActivationEntry {
            runtime: runtime.clone(),
            entry: source.entry.take(),
        },
    })
}

pub(crate) fn thaw(
    runtime: Runtime,
    kind: VmSuspendKind,
    resume_caller_realm: ContextId,
    data: &GeneratorActivationData,
    expected_function_kind: FunctionKind,
) -> Result<RootedVmActivation, RuntimeError> {
    #[cfg(feature = "profiling")]
    let _profile_phase = crate::engine::api::profiling::PhaseTimer::start_vm("thaw.decode");
    runtime.0.state.borrow().heap.context(resume_caller_realm)?;
    let bytecode_probe = FunctionBytecodeRef::from_borrowed_handle(runtime.clone(), data.bytecode)?;
    let executable = runtime.snapshot_function_bytecode(&bytecode_probe)?;
    let PublishedFunctionData {
        argument_definitions,
        local_definitions,
        metadata,
        realm,
        ..
    } = &*executable;
    let metadata = *metadata;
    let realm = *realm;
    drop(bytecode_probe);
    if metadata.function_kind != expected_function_kind
        || realm != data.vm.callee_realm
        || metadata.strict != data.vm.strict
        || data.arguments.len() < executable.frame_layout().arguments().len()
        || data.locals.len() != executable.frame_layout().locals().len()
        || data.reusable_captured_locals.len() != data.locals.len()
        || data.actual_argument_count > data.arguments.len()
        || data.original_arguments.len() != data.actual_argument_count
    {
        return Err(RuntimeError::Invariant(
            "raw resumable activation disagrees with published bytecode",
        ));
    }
    let current_function =
        ObjectRef::from_borrowed_handle(runtime.clone(), data.vm.current_function)?;
    let callable = runtime
        .as_callable(&current_function)?
        .ok_or(RuntimeError::Invariant(
            "resumable activation current function is not callable",
        ))?;
    let closure_slots = match runtime.bytecode_for_callable(&callable)? {
        CallableExecution::Bytecode {
            bytecode,
            closure_slots,
        } if bytecode.bytecode_id() == data.bytecode => closure_slots,
        CallableExecution::Bytecode { .. }
        | CallableExecution::Native { .. }
        | CallableExecution::Bound { .. }
        | CallableExecution::Proxy => {
            return Err(RuntimeError::Invariant(
                "resumable activation current function changed bytecode identity",
            ));
        }
    };
    if closure_slots.len() != executable.frame_layout().closures().len() {
        return Err(RuntimeError::Invariant(
            "resumable closure slot count disagrees with bytecode metadata",
        ));
    }
    let callee_global = ObjectRef::from_borrowed_handle(runtime.clone(), data.vm.callee_global)?;
    // Decode incrementally into an owning guard: a later rejection releases
    // every root already reconstructed instead of leaking the partial frame.
    let mut roots = super::stack::FrameStorageGuard::new(
        &runtime,
        super::stack::FrameStorage {
            original_arguments: Vec::new(),
            parameters: Vec::new(),
            locals: Vec::new(),
            operands: Vec::new(),
        },
    );
    {
        let storage = roots.storage_mut();
        for value in &data.original_arguments {
            storage
                .original_arguments
                .push(decode_raw_jsvalue(&runtime, value.clone())?);
        }
        for (index, binding) in data.arguments.iter().enumerate() {
            storage.parameters.push(decode_generator_frame_binding(
                &runtime,
                binding,
                argument_definitions.get(index),
            )?);
        }
        for (binding, definition) in data.locals.iter().zip(local_definitions.iter()) {
            storage.locals.push(decode_generator_frame_binding(
                &runtime,
                binding,
                Some(definition),
            )?);
        }
        for value in &data.vm.stack {
            storage
                .operands
                .push(decode_raw_jsvalue(&runtime, value.clone())?);
        }
    }
    if kind != VmSuspendKind::Initial
        && !matches!(
            roots.storage_mut().operands.last(),
            Some(JsValue::Undefined)
        )
    {
        return Err(RuntimeError::Invariant(
            "dormant suspension output was not cleared",
        ));
    }
    let this_value = decode_raw_jsvalue(&runtime, data.vm.this_value.clone())?;
    let new_target = match decode_raw_jsvalue(&runtime, data.vm.new_target.clone()) {
        Ok(value) => value,
        Err(error) => {
            let _ = runtime.release_jsvalue(this_value);
            return Err(error);
        }
    };
    let input = super::CallInput::new(&runtime, this_value, new_target, Some(callee_global));
    let normalized_this = data
        .vm
        .normalized_this
        .as_ref()
        .map(|value| decode_raw_jsvalue(&runtime, value.clone()))
        .transpose()?;
    let storage = roots.take();
    let mut entry = super::frame::FrameEntry {
        initialize_bindings: false,
        property_generation: 0,
        iterator_generation: 0,
        caller_realm: resume_caller_realm,
        active_frame: ActiveFrameToken(0),
        executable,
        cold: super::frame::ColdFrame::new(super::frame::FrameCold {
            rare: std::cell::OnceCell::new(),
            return_to: None,
            entry_guard: None,
            function: current_function.into(),
            closure_slots,
            reusable_captured_locals: data.reusable_captured_locals.clone(),
            input: input.into(),
        }),
        storage,
    };
    entry.cold.regions = data.vm.regions.clone();
    entry.cold.normalized_this = normalized_this;
    Ok(RootedVmActivation {
        entry: Some(entry),
        kind,
        saved_pc: data.vm.pc,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::api::Context;
    use crate::engine::heap::GeneratorState;
    use crate::engine::value::Value;

    fn dormant(context: &mut Context) -> (ObjectRef, GeneratorActivationData) {
        let Value::Object(generator) = context
            .eval("(function*(value) { let held = value; yield held; return held; })({marker:42})")
            .unwrap()
        else {
            panic!("expected a generator");
        };
        let (_, data) = context
            .runtime()
            .0
            .state
            .borrow()
            .heap
            .generator_snapshot(generator.object_id())
            .unwrap();
        (generator, data.unwrap())
    }

    #[test]
    fn suspended_generator_preserves_primitive_call_input_across_yields() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        assert_eq!(
            context.eval(r#"(()=>{
                function* values() { yield 1; yield 2; }
                for (const receiver of ['held string', 12345678901234567890n]) {
                    const iterator = values.call(receiver);
                    if (iterator.next().value !== 1 || iterator.next().value !== 2 || !iterator.next().done) return false;
                }
                String.prototype[Symbol.iterator] = values;
                return Uint32Array.from('anything').join(',') === '1,2';
            })()"#).unwrap(),
            crate::engine::value::Value::Bool(true),
        );
    }

    #[test]
    fn failed_thaw_releases_partial_roots_without_detaching_heap_state() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let (generator, data) = dormant(&mut context);
        let GeneratorFrameBinding::Direct(RawValue::Object(argument)) = data.arguments[0] else {
            panic!("expected an object argument");
        };
        let counts = || {
            let state = runtime.0.state.borrow();
            (
                state.heap.object_strong_count(argument).unwrap(),
                state
                    .heap
                    .object_strong_count(data.vm.current_function)
                    .unwrap(),
                state
                    .heap
                    .function_bytecode_strong_count(data.bytecode)
                    .unwrap(),
            )
        };
        let before = counts();
        // The first operand and all bindings acquire roots before decoding
        // rejects the internal-only second operand.
        let mut malformed = data.clone();
        malformed.vm.stack = vec![RawValue::Object(argument), RawValue::Exception];
        assert!(
            thaw(
                runtime.clone(),
                VmSuspendKind::Initial,
                context.realm,
                &malformed,
                FunctionKind::Generator
            )
            .is_err()
        );
        assert_eq!(counts(), before);
        runtime.run_gc().unwrap();
        let (state, after) = runtime
            .0
            .state
            .borrow()
            .heap
            .generator_snapshot(generator.object_id())
            .unwrap();
        assert_eq!(state, GeneratorState::SuspendedStart);
        // `GeneratorActivationData` has no `PartialEq` (its VM fields embed
        // `RawValue`), so equality is checked through the debug rendering.
        assert_eq!(
            after.as_ref().map(|entry| format!("{entry:?}")),
            Some(format!("{data:?}"))
        );
        assert!(runtime.0.state.borrow().active_frames.is_empty());
    }

    #[test]
    fn abandoned_thaw_does_not_register_or_keep_the_runtime_alive() {
        let runtime = Runtime::new();
        let weak = std::rc::Rc::downgrade(&runtime.0);
        let mut context = runtime.new_context();
        let (generator, data) = dormant(&mut context);
        let rooted = thaw(
            runtime.clone(),
            VmSuspendKind::Initial,
            context.realm,
            &data,
            FunctionKind::Generator,
        )
        .unwrap();
        assert!(runtime.0.state.borrow().active_frames.is_empty());
        drop(generator);
        drop(context);
        runtime.run_gc().unwrap();
        drop(runtime);
        assert!(weak.upgrade().is_some());
        drop(rooted);
        assert!(weak.upgrade().is_none());
    }

    #[test]
    fn resume_rejects_foreign_runtime_before_registering_a_frame() {
        let runtime = Runtime::new();
        let other = Runtime::new();
        let mut context = runtime.new_context();
        let (_generator, data) = dormant(&mut context);
        let rooted = thaw(
            runtime.clone(),
            VmSuspendKind::Initial,
            context.realm,
            &data,
            FunctionKind::Generator,
        )
        .unwrap();
        let result = rooted.run(&other, VmActivationResume::Initial);
        assert!(matches!(result, Err(RuntimeError::WrongRuntime(_))));
        assert!(runtime.0.state.borrow().active_frames.is_empty());
        assert!(other.0.state.borrow().active_frames.is_empty());
    }
}

#[cfg(all(test, feature = "profiling"))]
mod tests_owned;
