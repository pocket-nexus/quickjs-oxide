//! Differential coverage for public class async-generator methods.
//!
//! Private methods, `yield*`, `for await`, and return across an active
//! iterator remain separate fail-closed milestones. Destructuring parameters
//! remain outside this milestone's authenticated scope.

use crate::runtime_observation::host_runtime_context;
use crate::runtime_observation::{eval, text};

#[test]
fn suspended_class_methods_retain_home_objects_across_gc() {
    let (runtime, mut context) = host_runtime_context();
    drop(eval(
        &mut context,
        r#"
var instanceOutcome = "pending";
var staticOutcome = "pending";
var Base = class {
    get answer() {
        return this.seed + 2;
    }
    static get answer() {
        return this.seed + 2;
    }
};
var Derived = class extends Base {
    async *instance() {
        await 0;
        yield super.answer;
    }
    static async *method() {
        await 0;
        yield super.answer;
    }
};
Derived.seed = 40;
var instanceIterator = Derived.prototype.instance.call({ seed: 40 });
var staticIterator = Derived.method.call({ seed: 40 });
Derived = null;
Base = null;
instanceIterator.next().then(function (result) {
    instanceOutcome = result.value + ":" + result.done;
});
staticIterator.next().then(function (result) {
    staticOutcome = result.value + ":" + result.done;
});
"#,
    ));
    runtime.run_gc().unwrap();
    while runtime.is_job_pending().expect("runtime state") {
        runtime.execute_pending_job().unwrap();
        runtime.run_gc().unwrap();
    }
    assert_eq!(
        text(eval(&mut context, "instanceOutcome + '|' + staticOutcome")),
        "42:false|42:false"
    );
}
