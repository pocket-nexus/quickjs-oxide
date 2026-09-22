use crate::engine::api::{Runtime, Value};

#[test]
fn direct_store_handlers_preserve_assignment_values_and_aliases() {
    for source in [
        "(function(){var x={};var r=(x=x);return x===r})()",
        "(function(a){'use strict';var r=(a=a);return a===r})({})",
        "(function(){var x=new ArrayBuffer(16);x=42;return x===42})()",
        "(function(a){'use strict';a=new ArrayBuffer(16);return (a=42)===42})()",
        "(function(){var a=0,b=0,c='owned string';return (a=b=c)===c&&a===c&&b===c})()",
        "(function(a){'use strict';var b=0,c=123456789012345678901234567890n;return (a=b=c)===c&&a===c&&b===c})(0)",
        "(function(a){'use strict';var b=0,c=Symbol('store');a=c;b=a;return a===b&&b===c})(0)",
        "(function(){var n=0,x={},marker={},o={get value(){n++;return marker}};var r=(x=o.value);return n===1&&r===marker&&x===marker})()",
        "(function(){var x={};try{x=42;throw x}catch(e){return e===42&&x===42}})()",
    ] {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        assert_eq!(context.eval(source).unwrap(), Value::Bool(true), "{source}");
        assert!(runtime.0.state.borrow().active_frames.is_empty());
    }
}

#[test]
fn direct_store_handlers_keep_tdz_const_captured_and_mapped_argument_boundaries() {
    for source in [
        "(function(){var n=0;try{x=(n++,42);let x}catch(e){return e instanceof ReferenceError&&n===1}})()",
        "(function(){const x={};var original=x,n=0;try{x=(n++,{})}catch(e){return e instanceof TypeError&&n===1&&x===original}})()",
        "(function(){let x={};function read(){return x}var next={};var r=(x=next);return r===next&&read()===next})()",
        "(function(a){function read(){return a}var next={};a=next;return read()===next})({})",
        "(function(a){var next={};var r=(a=next);if(arguments[0]!==r)return false;arguments[0]=42;return a===42})({})",
        "(function(a){'use strict';var original=a;a=42;return arguments[0]===original&&a===42})({})",
        "(function(a=0){a=42;return arguments[0]===undefined&&a===42})()",
        "(function(){class C{#x=1;set(v){this.#x=v;return this.#x}}return new C().set(42)===42})()",
    ] {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        assert_eq!(context.eval(source).unwrap(), Value::Bool(true), "{source}");
        assert!(runtime.0.state.borrow().active_frames.is_empty());
    }
}

#[cfg(feature = "profiling")]
#[test]
fn direct_store_handlers_report_consume_and_keep_completions() {
    use crate::engine::api::profiling::CostProfile;

    let runtime = Runtime::new();
    let mut context = runtime.new_context();
    let profile = CostProfile::start();
    assert_eq!(
        context
            .eval("(function(a){'use strict';var x=0;x=41;a=x;return (x=a=42)===42&&x===a})(0)")
            .unwrap(),
        Value::Bool(true)
    );
    let costs = profile.snapshot();
    for event in ["direct_store.consume", "direct_store.keep"] {
        assert!(
            costs
                .owned_execution_events
                .get(event)
                .copied()
                .unwrap_or(0)
                > 0,
            "{event}: {costs:?}"
        );
    }
}
