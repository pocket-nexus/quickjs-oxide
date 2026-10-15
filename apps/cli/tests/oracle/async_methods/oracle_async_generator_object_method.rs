//! Differential coverage for ordinary object-literal async-generator methods.
//!
//! Class/private methods, `yield*`, `for await`, and return across an active
//! iterator remain separate fail-closed milestones. Destructuring parameters
//! remain outside this milestone's authenticated scope.

use crate::runtime_observation::host_runtime_context;
use crate::runtime_observation::{eval, text};

#[test]
fn suspended_method_retains_its_home_object_across_gc() {
    let (runtime, mut context) = host_runtime_context();
    drop(eval(
        &mut context,
        r#"
var outcome = "pending";
var base = {
    get answer() {
        return this.seed + 2;
    }
};
var holder = {
    __proto__: base,
    async *method() {
        await 0;
        yield super.answer;
    }
};
var iterator = holder.method.call({ seed: 40 });
holder = null;
base = null;
iterator.next().then(function (result) {
    outcome = result.value + ":" + result.done;
});
"#,
    ));
    runtime.run_gc().unwrap();
    while runtime.is_job_pending().expect("runtime state") {
        runtime.execute_pending_job().unwrap();
        runtime.run_gc().unwrap();
    }
    assert_eq!(text(eval(&mut context, "outcome")), "42:false");
}
