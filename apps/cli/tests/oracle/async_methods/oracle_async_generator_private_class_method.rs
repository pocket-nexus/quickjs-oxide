//! Differential coverage for private class async-generator methods.
//!
//! Delegation, `for await`, and `.return()` while a nested iterator is active
//! remain separate fail-closed milestones. Destructuring parameters remain
//! outside this milestone's authenticated scope.

use crate::runtime_observation::host_runtime_context;
use crate::runtime_observation::{eval, text};

#[test]
fn suspended_private_class_methods_retain_home_objects_and_brands_across_gc() {
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
    #offset = 2;
    async *#instance() {
        await 0;
        yield super.answer + this.#offset;
    }
    exposeInstance() {
        return this.#instance;
    }
    static #staticOffset = 2;
    static async *#method() {
        await 0;
        yield super.answer + this.#staticOffset;
    }
    static exposeMethod() {
        return this.#method;
    }
};
var instance = new Derived();
instance.seed = 38;
Derived.seed = 38;
var instanceMethod = instance.exposeInstance();
var staticMethod = Derived.exposeMethod();
var instanceIterator = instanceMethod.call(instance);
var staticIterator = staticMethod.call(Derived);
instanceMethod = null;
staticMethod = null;
instance = null;
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
