use super::*;

#[test]
fn can_block_policy_is_runtime_wide_and_isolated() {
    let runtime = Runtime::new();
    let clone = runtime.clone();
    let context = runtime.new_context().expect("create context");
    let other = Runtime::new();

    assert!(!runtime.can_block().expect("runtime configuration"));
    assert!(!clone.can_block().expect("runtime configuration"));
    assert!(
        !context
            .runtime()
            .can_block()
            .expect("runtime configuration")
    );
    assert!(!other.can_block().expect("runtime configuration"));

    clone
        .set_can_block(true)
        .expect("set runtime configuration");
    assert!(runtime.can_block().expect("runtime configuration"));
    assert!(clone.can_block().expect("runtime configuration"));
    assert!(
        context
            .runtime()
            .can_block()
            .expect("runtime configuration")
    );
    assert!(!other.can_block().expect("runtime configuration"));

    context
        .runtime()
        .set_can_block(false)
        .expect("set runtime configuration");
    assert!(!runtime.can_block().expect("runtime configuration"));
    assert!(!clone.can_block().expect("runtime configuration"));
    assert!(
        !context
            .runtime()
            .can_block()
            .expect("runtime configuration")
    );
}
