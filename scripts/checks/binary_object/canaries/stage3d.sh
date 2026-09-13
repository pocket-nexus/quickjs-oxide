expect_full_rewrite_table < "$boundary_dir/canaries/stage3d_canaries.txt"

# S03's explicitly selected owned core must preserve the same completion edge.
expect_full_rewrite_rejected owned-normal-completion-remapping \
    stage3d-throw-critical-route src/engine/vm/host_bridge.rs \
    'let result = owned::execute(host, input, arguments);' \
    'let result = owned::execute(host, input, arguments).map(|_| Completion::Return(Value::Undefined));'
expect_full_rewrite_rejected owned-normal-feature-gate-bypass \
    stage3d-throw-critical-route src/engine/vm/host_bridge.rs \
    $'#[cfg(feature = "stack-vm")]\n        let result = owned::execute(host, input, arguments);' \
    $'#[cfg(any())]\n        let result = owned::execute(host, input, arguments);'
