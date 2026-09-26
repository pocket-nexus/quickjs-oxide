//! Logical span and callsite observations for diagnostic builds.
//! These counters deliberately measure neither elapsed time nor retired code.

#[cfg(feature = "profiling")]
use super::current;
#[cfg(feature = "profiling")]
use crate::engine::api::runtime::Runtime;
#[cfg(feature = "profiling")]
use crate::engine::code::bytecode::Instruction;
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
pub struct FusionSiteKey {
    pub site: SiteKey,
    pub kind: &'static str,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FusionStaticCost {
    /// Lossy UTF-8 copies of the published source identity; they retain no
    /// `JsString`, Atom, bytecode or Runtime owner. Stripped debug data is None.
    pub function_name: Option<String>,
    pub filename: Option<String>,
    pub definition_line_zero_based: Option<u32>,
    pub definition_column_zero_based: Option<u32>,
    pub instructions: u64,
    pub direct_local_read_sites: u64,
    pub direct_argument_read_sites: u64,
    /// A direct local/argument read with no fusion flag at publication.
    pub unfused_read_sites: u64,
    pub dense_candidate_sites: u64,
    /// Direct local/argument reads that did not publish a dense span. Some
    /// may publish a different span kind.
    pub dense_noncandidate_read_sites: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FusionDispatchCost {
    pub visits: u64,
    /// Dynamic visits to a PC whose immutable fusion flag is zero.
    pub static_noncandidate_visits: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FusionSiteCost {
    pub attempts: u64,
    pub hits: u64,
    /// Guard misses preserve the canonical span. The `error` bucket denotes
    /// an exceptional termination and does not claim a canonical fallback.
    pub misses: BTreeMap<&'static str, u64>,
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
pub(crate) fn record_fusion_static(runtime: &Runtime, executable: &PublishedFunctionSnapshot) {
    let Some(collector) = current() else { return };
    let key = function_key(runtime, executable);
    let mut snapshot = collector.borrow_mut();
    if snapshot.fusion_static.contains_key(&key) {
        return;
    }
    if snapshot.fusion_static.len() == MAX_FUNCTIONS {
        snapshot.omitted_fusion_static_functions =
            snapshot.omitted_fusion_static_functions.saturating_add(1);
        return;
    }
    let (function_name, filename, definition_line_zero_based, definition_column_zero_based) =
        source_identity(runtime, executable);
    let mut cost = FusionStaticCost {
        function_name,
        filename,
        definition_line_zero_based,
        definition_column_zero_based,
        instructions: executable.code.len() as u64,
        ..FusionStaticCost::default()
    };
    for (pc, instruction) in executable.code.iter().enumerate() {
        let is_local = matches!(
            instruction,
            Instruction::GetLocal(_) | Instruction::GetLocalCheck(_)
        );
        let is_arg = matches!(instruction, Instruction::GetArg(_));
        if !is_local && !is_arg {
            continue;
        }
        cost.direct_local_read_sites += u64::from(is_local);
        cost.direct_argument_read_sites += u64::from(is_arg);
        cost.unfused_read_sites += u64::from(!executable.fusion.entry(pc).has_candidate());
        let dense = executable.fusion.dense_span(pc).is_some();
        cost.dense_candidate_sites += u64::from(dense);
        cost.dense_noncandidate_read_sites += u64::from(!dense);
    }
    snapshot.fusion_static.insert(key, cost);
}

#[cfg(feature = "profiling")]
pub(crate) fn record_fusion_dispatch(
    runtime: &Runtime,
    executable: &PublishedFunctionSnapshot,
    pc: usize,
    has_candidate: bool,
) {
    let Some(collector) = current() else { return };
    let key = site_key(runtime, executable, pc);
    let mut snapshot = collector.borrow_mut();
    if !snapshot.fusion_dispatch.contains_key(&key) && snapshot.fusion_dispatch.len() == MAX_SITES {
        snapshot.omitted_fusion_dispatch_events =
            snapshot.omitted_fusion_dispatch_events.saturating_add(1);
        return;
    }
    let cost = snapshot.fusion_dispatch.entry(key).or_default();
    cost.visits = cost.visits.saturating_add(1);
    cost.static_noncandidate_visits = cost
        .static_noncandidate_visits
        .saturating_add(u64::from(!has_candidate));
}

/// `miss == None` means the published candidate completed. Miss labels are
/// intentionally coarse. `error` denotes an exceptional termination rather
/// than a guard miss; it is still an attempt in the logical outcome total.
#[cfg(feature = "profiling")]
pub(crate) fn record_fusion_outcome(
    runtime: &Runtime,
    executable: &PublishedFunctionSnapshot,
    pc: usize,
    kind: &'static str,
    miss: Option<&'static str>,
) {
    let Some(collector) = current() else { return };
    let key = FusionSiteKey {
        site: site_key(runtime, executable, pc),
        kind,
    };
    let mut snapshot = collector.borrow_mut();
    if !snapshot.fusion_sites.contains_key(&key) && snapshot.fusion_sites.len() == MAX_SITES {
        snapshot.omitted_fusion_outcome_events =
            snapshot.omitted_fusion_outcome_events.saturating_add(1);
        return;
    }
    let cost = snapshot.fusion_sites.entry(key).or_default();
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
    fn dense_outcomes_include_guard_misses_and_static_noncandidate_visits() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        context.eval("function read(a,i){return a[i]}").unwrap();
        let profile = CostProfile::start();
        assert_eq!(context.eval("read([11],0)").unwrap(), Value::Int(11));
        assert_eq!(context.eval("read([11],'0')").unwrap(), Value::Int(11));
        assert_eq!(context.eval("read([11],-1)").unwrap(), Value::Undefined);
        assert_eq!(context.eval("read([11],2)").unwrap(), Value::Undefined);
        let costs = profile.snapshot();
        let dense = costs
            .fusion_sites
            .iter()
            .filter(|(key, _)| key.kind == "dense_read")
            .map(|(_, cost)| cost)
            .collect::<Vec<_>>();
        assert!(!dense.is_empty(), "{costs:?}");
        let hits: u64 = dense.iter().map(|cost| cost.hits).sum();
        assert!(hits >= 1, "{costs:?}");
        for reason in ["non_number", "index", "beyond_array_length"] {
            assert!(
                dense
                    .iter()
                    .any(|cost| cost.misses.get(reason).copied().unwrap_or(0) > 0),
                "missing {reason}: {costs:?}"
            );
        }
        for cost in costs.fusion_sites.values() {
            assert_eq!(
                cost.attempts,
                cost.hits + cost.misses.values().sum::<u64>(),
                "{cost:?}"
            );
        }
        assert!(
            costs
                .fusion_dispatch
                .values()
                .any(|cost| cost.static_noncandidate_visits > 0),
            "{costs:?}"
        );
        assert!(
            costs.fusion_static.values().any(
                |cost| cost.dense_candidate_sites > 0 && cost.dense_noncandidate_read_sites > 0
            ),
            "{costs:?}"
        );
        assert!(
            costs.fusion_static.values().any(|cost| {
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
