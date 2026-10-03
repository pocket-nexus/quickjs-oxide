//! Exercise the named property cache producer and its native call consumer.
use crate::engine::api::{
    Runtime, Value,
    profiling::{CostProfile, CostSnapshot},
};

fn observe(source: &str) -> CostSnapshot {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().expect("create context");
    // Resolve the lazy intrinsic before observing the repeated cache site.
    drop(context.eval("Math.min").unwrap());
    let profile = CostProfile::start();
    assert_eq!(context.eval(source).unwrap(), Value::Int(18));
    let costs = profile.snapshot();
    assert_eq!(runtime.0.active_frame_depth.get(), 0);
    assert!(runtime.0.state.borrow().active_frames.is_empty());
    assert!(!runtime.is_poisoned());
    costs
}

fn assert_cached_native_consumption(costs: &CostSnapshot) {
    assert!(
        costs
            .execution_sites
            .iter()
            .any(|(key, site)| { key.kind == "field_cache" && site.hits >= 8 }),
        "one actual named cache site must complete repeatedly: {:#?}",
        costs.execution_sites
    );
    assert!(
        costs
            .owned_execution_events
            .get("native_linked_classification_consumed")
            .copied()
            .unwrap_or(0)
            >= 8,
        "selected native metadata must reach its call consumer: {:#?}",
        costs.owned_execution_events
    );
    assert!(
        costs
            .owned_execution_events
            .get("property_selection.cache")
            .copied()
            .unwrap_or(0)
            >= 7,
        "the repeated named site must use its populated cache: {:#?}",
        costs.owned_execution_events
    );
}

#[test]
fn cached_named_native_facts_reach_the_call_consumer() {
    let costs = observe(
        "(function(){let holder=Math,total=0;for(let i=0;i<8;i++){total+=holder.min(i,3)}return total})()",
    );
    assert_cached_native_consumption(&costs);
}

#[test]
fn outer_native_hint_preserves_continuous_ordinary_argument_calls() {
    let costs = observe(
        "(function(){function ordinary(i){return i}let holder=Math,total=0;for(let i=0;i<8;i++){total+=holder.min(ordinary(i),3)}return total})()",
    );
    assert!(
        costs
            .owned_execution_events
            .get("core.internal_call")
            .copied()
            .unwrap_or(0)
            >= 8,
        "all eight ordinary argument calls must stay in the execution segment: {:#?}",
        costs.owned_execution_events
    );
    assert_cached_native_consumption(&costs);
}
