//! Logical span and callsite observations for diagnostic builds.
//! These counters deliberately measure neither elapsed time nor retired code.

#[cfg(feature = "profiling")]
use super::current;
#[cfg(feature = "profiling")]
use crate::engine::api::runtime::Runtime;
#[cfg(feature = "profiling")]
use crate::engine::code::exec_opcode::Opcode;
#[cfg(feature = "profiling")]
use crate::engine::code::runtime::PublishedFunctionSnapshot;
#[cfg(feature = "profiling")]
use crate::engine::heap::ObjectId;
#[cfg(feature = "profiling")]
use crate::engine::value::JsValue;
use std::collections::BTreeMap;

#[cfg(feature = "profiling")]
const MAX_FUNCTIONS: usize = 4_096;
#[cfg(feature = "profiling")]
const MAX_SITES: usize = 16_384;
#[cfg(feature = "profiling")]
const MAX_CALLEE_IDENTITIES: usize = 4;

/// The runtime domain and published bytecode generation identify one immutable
/// function without keeping either owner alive. `None` is reserved for
/// unpublished synthetic snapshots used by internal tests.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct FunctionSiteKey {
    pub runtime_id: u64,
    pub bytecode_id: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct SiteKey {
    pub function: FunctionSiteKey,
    pub pc: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ExecutionSiteKey {
    pub site: SiteKey,
    pub kind: &'static str,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ExecutionStaticCost {
    /// Lossy UTF-8 copies of the published source identity; they retain no
    /// `JsString`, Atom, bytecode or Runtime owner. Stripped debug data is None.
    pub function_name: Option<String>,
    pub filename: Option<String>,
    pub definition_line_zero_based: Option<u32>,
    pub definition_column_zero_based: Option<u32>,
    pub instructions: u64,
    pub direct_local_read_sites: u64,
    pub direct_argument_read_sites: u64,
    pub specialized_number_read_sites: u64,
    pub cached_field_read_sites: u64,
    pub dense_array_read_sites: u64,
    /// Direct local/argument reads published as generic opcodes.
    pub generic_read_sites: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ExecutionDispatchCost {
    pub visits: u64,
    /// Dynamic visits to generic local/argument read opcodes.
    pub generic_visits: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ExecutionSiteCost {
    pub attempts: u64,
    pub hits: u64,
    /// Guard misses preserve the canonical span. The `error` bucket denotes
    /// an exceptional termination and does not claim a canonical fallback.
    pub misses: BTreeMap<&'static str, u64>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NumericRejectionCost {
    pub source_pc: u32,
    pub family: &'static str,
    pub reason: &'static str,
    pub lowered_window: String,
    pub visits: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CallsiteCost {
    pub calls: u64,
    pub object_callees: u64,
    pub nonobject_callees: u64,
    /// Changes between consecutive object callee identities. A nonobject
    /// callee breaks the chain and is counted separately above.
    pub callee_identity_changes: u64,
    /// Exact distinct count until the first four identities; thereafter this
    /// is a lower bound and `distinct_overflow` is true.
    pub distinct_callees_observed: u8,
    pub distinct_overflow: bool,
    #[cfg(feature = "profiling")]
    last: Option<ObjectId>,
    #[cfg(feature = "profiling")]
    seen: Vec<ObjectId>,
}

#[cfg(feature = "profiling")]
impl CallsiteCost {
    fn observe(&mut self, value: &JsValue) {
        self.calls = self.calls.saturating_add(1);
        let JsValue::Object(id) = value else {
            self.nonobject_callees = self.nonobject_callees.saturating_add(1);
            self.last = None;
            return;
        };
        self.object_callees = self.object_callees.saturating_add(1);
        if self.last.is_some_and(|last| last != *id) {
            self.callee_identity_changes = self.callee_identity_changes.saturating_add(1);
        }
        self.last = Some(*id);
        if !self.seen.contains(id) {
            if self.seen.len() < MAX_CALLEE_IDENTITIES {
                self.seen.push(*id);
                self.distinct_callees_observed = self.seen.len() as u8;
            } else {
                self.distinct_overflow = true;
            }
        }
    }
}

#[cfg(feature = "profiling")]
fn function_key(runtime: &Runtime, executable: &PublishedFunctionSnapshot) -> FunctionSiteKey {
    FunctionSiteKey {
        runtime_id: runtime.domain_id(),
        bytecode_id: executable.bytecode_id().map(|id| id.publish_generation()),
    }
}

#[cfg(feature = "profiling")]
fn site_key(runtime: &Runtime, executable: &PublishedFunctionSnapshot, pc: usize) -> SiteKey {
    SiteKey {
        function: function_key(runtime, executable),
        pc,
    }
}

#[cfg(feature = "profiling")]
fn source_identity(
    runtime: &Runtime,
    executable: &PublishedFunctionSnapshot,
) -> (Option<String>, Option<String>, Option<u32>, Option<u32>) {
    let _diagnostic = super::core::DiagnosticScope::enter();
    let Some(id) = executable.bytecode_id() else {
        return (None, None, None, None);
    };
    // A profiling probe must not turn an active Runtime borrow into a panic.
    let Ok(state) = runtime.0.state.try_borrow() else {
        return (None, None, None, None);
    };
    let Ok(bytecode) = state.heap.function_bytecode(id) else {
        return (None, None, None, None);
    };
    let name = bytecode.func_name.as_ref().map(|name| name.to_utf8_lossy());
    let filename = bytecode.debug.as_ref().and_then(|debug| {
        state
            .atoms
            .to_js_string(debug.filename)
            .ok()
            .map(|filename| filename.to_utf8_lossy())
    });
    let position = bytecode
        .debug
        .as_ref()
        .and_then(|debug| debug.pc2line.as_ref())
        .map(|table| table.definition);
    (
        name,
        filename,
        position.map(|position| position.line),
        position.map(|position| position.column),
    )
}

/// Register one executed immutable function's direct-read candidate inventory.
/// Repeated calls at frame entry only perform a map lookup in diagnostic builds.
#[cfg(feature = "profiling")]
pub(crate) fn record_execution_static(runtime: &Runtime, executable: &PublishedFunctionSnapshot) {
    let Some(collector) = current() else { return };
    let key = function_key(runtime, executable);
    let mut snapshot = collector.borrow_mut();
    if snapshot.execution_static.contains_key(&key) {
        return;
    }
    if snapshot.execution_static.len() == MAX_FUNCTIONS {
        snapshot.omitted_execution_static_functions = snapshot
            .omitted_execution_static_functions
            .saturating_add(1);
        return;
    }
    let (function_name, filename, definition_line_zero_based, definition_column_zero_based) =
        source_identity(runtime, executable);
    let mut cost = ExecutionStaticCost {
        function_name,
        filename,
        definition_line_zero_based,
        definition_column_zero_based,
        instructions: executable.exec.instruction_len() as u64,
        ..ExecutionStaticCost::default()
    };
    for pc in 0..executable.exec.instruction_len() {
        let Some(opcode) = executable.exec.opcode_at_source(pc) else {
            continue;
        };
        let is_local = matches!(
            opcode,
            Opcode::GetLocal
                | Opcode::GetLocalCheck
                | Opcode::NumberLocalInc
                | Opcode::NumericArrayAccumulate
                | Opcode::DensePreUpdateLocal
                | Opcode::DenseReadLocal
                | Opcode::BorrowedFieldLocal
                | Opcode::CompareBranchLocal
                | Opcode::CompareBranchLocalLt
                | Opcode::UpdateLocalDiscard
                | Opcode::UpdateLocalDiscardCheck
                | Opcode::DensePostUpdateLocal
                | Opcode::DensePostUpdateLocalCheck
                | Opcode::DenseReadBinaryLocal
                | Opcode::DenseIndexBinaryLocal
                | Opcode::DenseAccIndexSetDrop
                | Opcode::FieldAccSetDrop
        );
        let is_arg = matches!(
            opcode,
            Opcode::GetArg
                | Opcode::NumberArgInc
                | Opcode::DensePreUpdateArg
                | Opcode::DenseReadArg
                | Opcode::BorrowedFieldArg
                | Opcode::CompareBranchArg
                | Opcode::CompareBranchArgLt
                | Opcode::DensePostUpdateArg
                | Opcode::DenseReadBinaryArg
                | Opcode::DenseIndexBinaryArg
        );
        cost.cached_field_read_sites += u64::from(matches!(
            opcode,
            Opcode::GetFieldCached
                | Opcode::GetField2Cached
                | Opcode::BorrowedFieldLocal
                | Opcode::BorrowedFieldArg
                | Opcode::BorrowedFieldThis
                | Opcode::FieldAccSetDrop
        ));
        cost.dense_array_read_sites += u64::from(matches!(
            opcode,
            Opcode::GetArrayElDense
                | Opcode::GetArrayEl2Dense
                | Opcode::GetArrayEl3Dense
                | Opcode::DensePreUpdateLocal
                | Opcode::DensePreUpdateArg
                | Opcode::DenseReadLocal
                | Opcode::DenseReadArg
                | Opcode::DensePostUpdateLocal
                | Opcode::DensePostUpdateLocalCheck
                | Opcode::DensePostUpdateArg
                | Opcode::DenseReadBinaryLocal
                | Opcode::DenseReadBinaryArg
                | Opcode::DenseIndexBinaryLocal
                | Opcode::DenseIndexBinaryArg
                | Opcode::DenseAccIndexSetDrop
                | Opcode::NumericArrayAccumulate
        ));
        if !is_local && !is_arg {
            continue;
        }
        cost.direct_local_read_sites += u64::from(is_local);
        cost.direct_argument_read_sites += u64::from(is_arg);
        let specialized = matches!(
            opcode,
            Opcode::NumberLocalInc
                | Opcode::NumericArrayAccumulate
                | Opcode::NumberArgInc
                | Opcode::DensePreUpdateLocal
                | Opcode::DensePreUpdateArg
                | Opcode::DenseReadLocal
                | Opcode::DenseReadArg
                | Opcode::BorrowedFieldLocal
                | Opcode::BorrowedFieldArg
                | Opcode::CompareBranchLocal
                | Opcode::CompareBranchArg
                | Opcode::CompareBranchLocalLt
                | Opcode::CompareBranchArgLt
                | Opcode::UpdateLocalDiscard
                | Opcode::UpdateLocalDiscardCheck
                | Opcode::DensePostUpdateLocal
                | Opcode::DensePostUpdateLocalCheck
                | Opcode::DensePostUpdateArg
                | Opcode::DenseReadBinaryLocal
                | Opcode::DenseReadBinaryArg
                | Opcode::DenseIndexBinaryLocal
                | Opcode::DenseIndexBinaryArg
                | Opcode::DenseAccIndexSetDrop
                | Opcode::FieldAccSetDrop
        );
        cost.specialized_number_read_sites += u64::from(specialized);
        cost.generic_read_sites += u64::from(!specialized);
    }
    snapshot.execution_static.insert(key, cost);
    for site in executable.exec.rejected_numeric_sites() {
        if snapshot.numeric_rejections.len() == MAX_SITES {
            snapshot.omitted_numeric_rejection_sites =
                snapshot.omitted_numeric_rejection_sites.saturating_add(1);
            continue;
        }
        snapshot.numeric_rejections.insert(
            SiteKey {
                function: key,
                pc: site.pc as usize,
            },
            NumericRejectionCost {
                source_pc: site.source_pc,
                family: site.family,
                reason: site.reason,
                lowered_window: site.lowered_window.clone(),
                visits: 0,
            },
        );
    }
}

#[cfg(feature = "profiling")]
pub(crate) fn record_numeric_rejection_visit(
    runtime: &Runtime,
    executable: &PublishedFunctionSnapshot,
    pc: usize,
) {
    let sites = executable.exec.rejected_numeric_sites();
    if sites
        .binary_search_by_key(&(pc as u32), |site| site.pc)
        .is_err()
    {
        return;
    }
    let Some(collector) = current() else { return };
    let key = site_key(runtime, executable, pc);
    if let Some(site) = collector.borrow_mut().numeric_rejections.get_mut(&key) {
        site.visits = site.visits.saturating_add(1);
    }
}

#[cfg(feature = "profiling")]
pub(crate) fn record_execution_dispatch(
    runtime: &Runtime,
    executable: &PublishedFunctionSnapshot,
    pc: usize,
    has_candidate: bool,
) {
    let Some(collector) = current() else { return };
    let key = site_key(runtime, executable, pc);
    let mut snapshot = collector.borrow_mut();
    if !snapshot.execution_dispatch.contains_key(&key)
        && snapshot.execution_dispatch.len() == MAX_SITES
    {
        snapshot.omitted_execution_dispatch_events =
            snapshot.omitted_execution_dispatch_events.saturating_add(1);
        return;
    }
    let cost = snapshot.execution_dispatch.entry(key).or_default();
    cost.visits = cost.visits.saturating_add(1);
    cost.generic_visits = cost
        .generic_visits
        .saturating_add(u64::from(!has_candidate));
}

/// `miss == None` means the published candidate completed. Miss labels are
/// intentionally coarse. `error` denotes an exceptional termination rather
/// than a guard miss; it is still an attempt in the logical outcome total.
#[cfg(feature = "profiling")]
pub(crate) fn record_execution_outcome(
    runtime: &Runtime,
    executable: &PublishedFunctionSnapshot,
    pc: usize,
    kind: &'static str,
    miss: Option<&'static str>,
) {
    let Some(collector) = current() else { return };
    let key = ExecutionSiteKey {
        site: site_key(runtime, executable, pc),
        kind,
    };
    let mut snapshot = collector.borrow_mut();
    if !snapshot.execution_sites.contains_key(&key) && snapshot.execution_sites.len() == MAX_SITES {
        snapshot.omitted_execution_outcome_events =
            snapshot.omitted_execution_outcome_events.saturating_add(1);
        return;
    }
    let cost = snapshot.execution_sites.entry(key).or_default();
    cost.attempts = cost.attempts.saturating_add(1);
    if let Some(reason) = miss {
        let count = cost.misses.entry(reason).or_default();
        *count = count.saturating_add(1);
    } else {
        cost.hits = cost.hits.saturating_add(1);
    }
}

#[cfg(feature = "profiling")]
pub(crate) fn record_callsite_callee(
    runtime: &Runtime,
    executable: &PublishedFunctionSnapshot,
    pc: usize,
    callee: &JsValue,
) {
    let Some(collector) = current() else { return };
    let key = site_key(runtime, executable, pc);
    let mut snapshot = collector.borrow_mut();
    if !snapshot.callsites.contains_key(&key) && snapshot.callsites.len() == MAX_SITES {
        snapshot.omitted_callsite_events = snapshot.omitted_callsite_events.saturating_add(1);
        return;
    }
    snapshot.callsites.entry(key).or_default().observe(callee);
}

#[cfg(all(test, feature = "profiling"))]
mod tests {
    use super::super::CostProfile;
    use crate::engine::api::{Runtime, Value};

    #[test]
    fn execution_word_outcomes_separate_dense_hits_and_generic_reads() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let _ = context
            .eval("function read(a,i){return a[i]}; let array=[11]")
            .unwrap();
        let profile = CostProfile::start();
        assert_eq!(context.eval("read(array,0)").unwrap(), Value::Int(11));
        assert_eq!(context.eval("read(array,'0')").unwrap(), Value::Int(11));
        assert_eq!(context.eval("read(array,-1)").unwrap(), Value::Undefined);
        assert_eq!(context.eval("read(array,2)").unwrap(), Value::Undefined);
        let costs = profile.snapshot();
        let dense = costs
            .execution_sites
            .iter()
            .filter(|(key, _)| key.kind == "dense_array_read")
            .map(|(_, cost)| cost)
            .collect::<Vec<_>>();
        assert!(!dense.is_empty(), "{costs:?}");
        let hits: u64 = dense.iter().map(|cost| cost.hits).sum();
        assert!(hits >= 1, "{costs:?}");
        assert!(
            dense
                .iter()
                .any(|cost| cost.misses.get("guard").copied().unwrap_or(0) > 0),
            "{costs:?}"
        );
        for cost in costs.execution_sites.values() {
            assert_eq!(
                cost.attempts,
                cost.hits + cost.misses.values().sum::<u64>(),
                "{cost:?}"
            );
        }
        assert!(
            costs
                .execution_dispatch
                .values()
                .any(|cost| cost.generic_visits > 0),
            "{costs:?}"
        );
        assert!(
            costs.execution_static.values().any(|cost| {
                cost.dense_array_read_sites > 0 && cost.direct_argument_read_sites > 0
            }),
            "{costs:?}"
        );
        assert!(
            costs.execution_static.values().any(|cost| {
                cost.function_name.as_deref() == Some("read")
                    && cost.filename.is_some()
                    && cost.definition_line_zero_based.is_some()
            }),
            "{costs:?}"
        );
    }

    #[test]
    fn callsite_callee_history_is_bounded_without_owning_functions() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let profile = CostProfile::start();
        assert_eq!(
            context
                .eval("(function(){function invoke(f){return f()}var s=0;for(var i=0;i<9;i++){s+=invoke(function(){return i})}return s})()")
                .unwrap(),
            Value::Int(36)
        );
        drop(context);
        drop(runtime);
        let costs = profile.snapshot();
        let (_, cost) = costs
            .callsites
            .iter()
            .find(|(_, cost)| cost.object_callees >= 9 && cost.distinct_overflow)
            .unwrap_or_else(|| panic!("missing polymorphic callsite: {costs:?}"));
        assert_eq!(cost.distinct_callees_observed, 4);
        assert_eq!(cost.seen.len(), 4);
        assert!(cost.callee_identity_changes >= 8);
    }
}
