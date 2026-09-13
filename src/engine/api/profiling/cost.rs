//! Scoped thread-local diagnostics, independent of Runtime ownership.
//!
//! Only the innermost live scope receives events. Guards hold no Runtime or JS
//! values, and dropping a guard (including during unwinding) unregisters it.
//! Timing is inclusive wall time, including nested compilation/host callbacks;
//! phase totals must not be added to obtain exclusive compile time.

use std::cell::RefCell;
use std::rc::{Rc, Weak};
use std::time::Instant;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PhaseCost {
    /// Entries, including operations which return an error or unwind.
    pub attempts: u64,
    pub inclusive_ns: u128,
    /// Boundary snapshots, not allocations or continuous memory sampling.
    pub storage_samples: u64,
    /// Largest partial owned-Vec capacity snapshot in this phase. Excludes
    /// referenced payloads and temporary worklists; not a compiler peak total.
    pub maximum_observed_ir_capacity_bytes: u64,
}

/// Partial owned-core costs. Capacity peaks are per store, not process totals.
/// Moves count logical owning transfers, not machine instructions or bytes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OwnedStorageCost {
    pub slot_capacity_growths: u64,
    pub maximum_slot_capacity: usize,
    pub frame_capacity_growths: u64,
    pub maximum_frame_capacity: usize,
    pub frames_pushed: u64,
    pub maximum_frame_depth: usize,
    pub slots_initialized: u64,
    pub maximum_reserved_slots: usize,
    pub maximum_live_slots: usize,
    pub slot_moves: u64,
    pub slot_clears: u64,
    pub value_copies: u64,
    /// Object/Symbol retains at the slot copy boundary; excludes primitive Rc.
    pub copied_heap_roots: u64,
    pub hot_value_releases: u64,
    /// Object/Symbol releases proven safe by the narrow slot transaction.
    pub hot_heap_root_releases: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CostSnapshot {
    pub parse: PhaseCost,
    pub resolution: PhaseCost,
    pub lowering: PhaseCost,
    /// Successfully lowered function drafts, including nested functions.
    /// This is not a count of published functions or a unique-code inventory.
    pub lowered_functions: u64,
    pub code_instructions: u64,
    /// Inline typed-instruction storage; excludes boxed operands and metadata.
    pub code_inline_bytes: u64,
    pub maximum_verified_stack: u16,
    /// Optional final-code dumps, in lowering completion order. Disabled by
    /// default; these retain text only, not Runtime roots or IR storage.
    pub code_disassembly: Option<Vec<String>>,
    /// Actual entries to the previous interpreter's instruction dispatch.
    pub legacy_dispatches: u64,
    pub legacy_pc_publications: u64,
    /// Operand depth at dispatch boundaries, excluding locals/arguments.
    pub legacy_max_operand_depth: usize,
    /// Successfully completed instructions in the owned ordinary-call core.
    pub owned_instructions: u64,
    /// Untouched instructions handed to the temporary previous-VM bridge.
    pub owned_bridge_exits: u64,
    pub owned_max_operand_depth: usize,
    pub owned_storage: OwnedStorageCost,
}

type Collector = RefCell<CostSnapshot>;
thread_local! {
    static SCOPES: RefCell<Vec<Weak<Collector>>> = const { RefCell::new(Vec::new()) };
}

/// A diagnostic collection interval on the calling thread. Not Send or Sync.
/// Production builds without the `profiling` feature contain no event hooks.
pub struct CostProfile {
    collector: Rc<Collector>,
}

impl CostProfile {
    #[must_use]
    pub fn start() -> Self {
        let collector = Rc::new(RefCell::new(CostSnapshot::default()));
        SCOPES.with(|scopes| scopes.borrow_mut().push(Rc::downgrade(&collector)));
        Self { collector }
    }

    /// Capture disassembly for subsequent successfully lowered drafts.
    /// Existing captures are preserved. Intended for diagnostics, not timing.
    pub fn capture_disassembly(&self) {
        self.collector
            .borrow_mut()
            .code_disassembly
            .get_or_insert_with(Vec::new);
    }

    #[must_use]
    pub fn snapshot(&self) -> CostSnapshot {
        self.collector.borrow().clone()
    }
}

impl Drop for CostProfile {
    fn drop(&mut self) {
        SCOPES.with(|scopes| {
            scopes
                .borrow_mut()
                .retain(|scope| !scope.ptr_eq(&Rc::downgrade(&self.collector)));
        });
    }
}

fn current() -> Option<Rc<Collector>> {
    SCOPES.with(|scopes| scopes.borrow().last().and_then(Weak::upgrade))
}

#[derive(Clone, Copy)]
pub(crate) enum CompilePhase {
    Parse,
    Resolution,
    Lowering,
}

pub(crate) struct PhaseTimer {
    active: Option<(Rc<Collector>, Instant, CompilePhase)>,
}

impl PhaseTimer {
    pub(crate) fn start(phase: CompilePhase) -> Self {
        Self {
            active: current().map(|collector| (collector, Instant::now(), phase)),
        }
    }
}

impl Drop for PhaseTimer {
    fn drop(&mut self) {
        let Some((collector, start, phase)) = &self.active else {
            return;
        };
        let elapsed = start.elapsed().as_nanos();
        let mut costs = collector.borrow_mut();
        let cost = match phase {
            CompilePhase::Parse => &mut costs.parse,
            CompilePhase::Resolution => &mut costs.resolution,
            CompilePhase::Lowering => &mut costs.lowering,
        };
        cost.attempts = cost.attempts.saturating_add(1);
        cost.inclusive_ns = cost.inclusive_ns.saturating_add(elapsed);
    }
}

pub(crate) fn cost_profile_active() -> bool {
    current().is_some()
}

pub(crate) fn record_compiler_storage(phase: CompilePhase, bytes: u64) {
    if let Some(collector) = current() {
        let mut costs = collector.borrow_mut();
        let cost = match phase {
            CompilePhase::Parse => &mut costs.parse,
            CompilePhase::Resolution => &mut costs.resolution,
            CompilePhase::Lowering => &mut costs.lowering,
        };
        cost.storage_samples = cost.storage_samples.saturating_add(1);
        cost.maximum_observed_ir_capacity_bytes =
            cost.maximum_observed_ir_capacity_bytes.max(bytes);
    }
}

pub(crate) fn record_lowered_function(
    code: &[crate::engine::code::bytecode::Instruction],
    max_stack: u16,
) {
    let instructions = code.len();
    if let Some(collector) = current() {
        let mut costs = collector.borrow_mut();
        costs.lowered_functions = costs.lowered_functions.saturating_add(1);
        costs.code_instructions = costs.code_instructions.saturating_add(instructions as u64);
        costs.code_inline_bytes = costs.code_inline_bytes.saturating_add(
            instructions.saturating_mul(size_of::<crate::engine::code::bytecode::Instruction>())
                as u64,
        );
        costs.maximum_verified_stack = costs.maximum_verified_stack.max(max_stack);
        if let Some(dumps) = &mut costs.code_disassembly {
            dumps.push(crate::engine::code::instruction::disassemble(code));
        }
    }
}

pub(crate) fn record_legacy_dispatch(operand_depth: usize) {
    if let Some(collector) = current() {
        let mut costs = collector.borrow_mut();
        costs.legacy_dispatches = costs.legacy_dispatches.saturating_add(1);
        costs.legacy_max_operand_depth = costs.legacy_max_operand_depth.max(operand_depth);
    }
}

pub(crate) fn record_legacy_pc_publication() {
    if let Some(collector) = current() {
        let mut costs = collector.borrow_mut();
        costs.legacy_pc_publications = costs.legacy_pc_publications.saturating_add(1);
    }
}

#[cfg(feature = "stack-vm")]
pub(crate) fn record_owned_instruction(operand_depth: usize) {
    if let Some(collector) = current() {
        let mut costs = collector.borrow_mut();
        costs.owned_instructions = costs.owned_instructions.saturating_add(1);
        costs.owned_max_operand_depth = costs.owned_max_operand_depth.max(operand_depth);
    }
}

#[cfg(feature = "stack-vm")]
pub(crate) fn record_owned_bridge() {
    if let Some(collector) = current() {
        let mut costs = collector.borrow_mut();
        costs.owned_bridge_exits = costs.owned_bridge_exits.saturating_add(1);
    }
}

#[cfg(feature = "stack-vm")]
pub(crate) enum OwnedStorageEvent {
    SlotCapacity { before: usize, after: usize },
    FrameCapacity { before: usize, after: usize },
    FramePush(usize),
    Initialize(usize),
    Occupancy { reserved: usize, live: usize },
    Move(usize),
    Clear(usize),
    Copy { heap_root: bool },
    HotRelease { heap_root: bool },
}

#[cfg(feature = "stack-vm")]
pub(crate) fn record_owned_storage(event: OwnedStorageEvent) {
    let Some(collector) = current() else {
        return;
    };
    let mut snapshot = collector.borrow_mut();
    let cost = &mut snapshot.owned_storage;
    match event {
        OwnedStorageEvent::SlotCapacity { before, after } => {
            cost.slot_capacity_growths = cost
                .slot_capacity_growths
                .saturating_add(u64::from(after > before));
            cost.maximum_slot_capacity = cost.maximum_slot_capacity.max(after);
        }
        OwnedStorageEvent::FrameCapacity { before, after } => {
            cost.frame_capacity_growths = cost
                .frame_capacity_growths
                .saturating_add(u64::from(after > before));
            cost.maximum_frame_capacity = cost.maximum_frame_capacity.max(after);
        }
        OwnedStorageEvent::FramePush(depth) => {
            cost.frames_pushed = cost.frames_pushed.saturating_add(1);
            cost.maximum_frame_depth = cost.maximum_frame_depth.max(depth);
        }
        OwnedStorageEvent::Initialize(count) => {
            cost.slots_initialized = cost.slots_initialized.saturating_add(count as u64)
        }
        OwnedStorageEvent::Occupancy { reserved, live } => {
            cost.maximum_reserved_slots = cost.maximum_reserved_slots.max(reserved);
            cost.maximum_live_slots = cost.maximum_live_slots.max(live);
        }
        OwnedStorageEvent::Move(count) => {
            cost.slot_moves = cost.slot_moves.saturating_add(count as u64)
        }
        OwnedStorageEvent::Clear(count) => {
            cost.slot_clears = cost.slot_clears.saturating_add(count as u64)
        }
        OwnedStorageEvent::Copy { heap_root } => {
            cost.value_copies = cost.value_copies.saturating_add(1);
            cost.copied_heap_roots = cost.copied_heap_roots.saturating_add(u64::from(heap_root));
        }
        OwnedStorageEvent::HotRelease { heap_root } => {
            cost.hot_value_releases = cost.hot_value_releases.saturating_add(1);
            cost.hot_heap_root_releases = cost
                .hot_heap_root_releases
                .saturating_add(u64::from(heap_root));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::CostProfile;
    use crate::engine::api::{Runtime, Value};

    #[test]
    fn real_compile_and_execution_are_counted_and_nested_scopes_are_isolated() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let outer = CostProfile::start();
        assert_eq!(
            context.eval("(function(x){return x+1;})(41)").unwrap(),
            Value::Int(42)
        );
        let before = outer.snapshot();
        assert_eq!(before.parse.attempts, 1);
        assert_eq!(before.resolution.attempts, 1);
        assert_eq!(before.lowering.attempts, 1);
        assert_eq!(before.lowered_functions, 2);
        assert_eq!(before.parse.storage_samples, 1);
        assert_eq!(before.resolution.storage_samples, 2);
        assert_eq!(before.lowering.storage_samples, 1);
        assert!(before.parse.maximum_observed_ir_capacity_bytes > 0);
        assert!(before.code_instructions > 0);
        if cfg!(feature = "stack-vm") {
            assert!(before.owned_instructions > 0);
        } else {
            assert!(before.legacy_dispatches > 0);
            assert_eq!(before.owned_instructions, 0);
            assert_eq!(before.owned_bridge_exits, 0);
        }
        assert_eq!(before.legacy_dispatches, before.legacy_pc_publications);
        {
            let inner = CostProfile::start();
            context.eval("1+2").unwrap();
            assert_eq!(inner.snapshot().parse.attempts, 1);
            assert_eq!(outer.snapshot(), before);
        }
        context.eval("3+4").unwrap();
        assert_eq!(outer.snapshot().parse.attempts, 2);
    }

    #[test]
    fn parse_failure_and_unwinding_release_the_collection_scope() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let outer = CostProfile::start();
        assert!(context.eval("let = ;").is_err());
        let failed = outer.snapshot();
        assert_eq!(failed.parse.attempts, 1);
        assert_eq!(failed.resolution.attempts, 0);
        assert_eq!(failed.lowered_functions, 0);
        let unwind = std::panic::catch_unwind(|| {
            let _inner = CostProfile::start();
            panic!("profile unwind probe");
        });
        assert!(unwind.is_err());
        context.eval("42").unwrap();
        assert_eq!(outer.snapshot().parse.attempts, 2);
    }
}

#[cfg(test)]
mod disassembly_tests {
    use super::CostProfile;
    use crate::engine::api::{Runtime, Value};

    #[test]
    fn optional_disassembly_captures_only_subsequent_final_code() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let profile = CostProfile::start();
        context.eval("0").unwrap();
        let before = profile.snapshot();
        assert!(before.code_disassembly.is_none());
        profile.capture_disassembly();
        assert_eq!(
            context.eval("(function(x){return x+1;})(41)").unwrap(),
            Value::Int(42)
        );
        let after = profile.snapshot();
        let dumps = after.code_disassembly.as_ref().unwrap();
        assert_eq!(
            dumps.len() as u64,
            after.lowered_functions - before.lowered_functions
        );
        assert_eq!(
            dumps
                .iter()
                .map(|dump| dump.lines().count() as u64)
                .sum::<u64>(),
            after.code_instructions - before.code_instructions
        );
        let add = dumps
            .iter()
            .flat_map(|dump| dump.lines())
            .find(|line| line.contains(" Add ; "))
            .unwrap();
        assert!(add.contains("may_call_js: true"));
        assert!(add.contains("may_allocate: true"));
        profile.capture_disassembly();
        assert_eq!(profile.snapshot(), after);
    }
}
