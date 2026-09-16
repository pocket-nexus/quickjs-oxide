# S13 replacement for retired VmHost dispatch/feature-gate canaries.
expect_full_rewrite_rejected owned-root-throw-completion \
    s13-owned-route src/engine/vm/root_call.rs \
    'result.finish(self.clone()).map_err(RuntimeError::Engine)' \
    'result.finish(self.clone()).map(|_| Completion::Return(Value::Undefined)).map_err(RuntimeError::Engine)'
expect_full_rewrite_rejected owned-frame-forwarded-completion \
    s13-owned-route src/engine/vm/frame_exit.rs \
    'Some(completion) => completion,' \
    'Some(_) => Completion::Return(Value::Undefined),'
