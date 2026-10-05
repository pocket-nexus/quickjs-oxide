use super::*;

#[test]
fn failed_final_teardown_abandons_state_before_later_owned_cleanup() {
    for case in ["deferred", "kept", "job"] {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "engine::heap::runtime::tests::teardown::teardown_child",
                "--nocapture",
            ])
            .env("QJS_FINAL_TEARDOWN_CHILD", case)
            .env("QJS_TEARDOWN_PROBE", "1")
            .status()
            .unwrap();
        assert!(
            status.success(),
            "final teardown subprocess {case}: {status}"
        );
    }
}

#[test]
fn teardown_child() {
    let Ok(case) = std::env::var("QJS_FINAL_TEARDOWN_CHILD") else {
        return;
    };
    let runtime = Runtime::new();
    let runtime_weak = std::rc::Rc::downgrade(&runtime.0);
    let stale = runtime.new_object(None).unwrap().into_handle();
    let string = JsString::from_static("later teardown owner");
    let string_weak = string.downgrade();
    let JsValue::String(string) = runtime.into_jsvalue(Value::String(string)).unwrap() else {
        panic!("owned string");
    };
    let job = if case == "job" {
        let context = runtime.new_context().unwrap();
        let realm = context.realm_id();
        let callback = runtime.new_object(None).unwrap().into_handle();
        runtime
            .0
            .state
            .borrow_mut()
            .heap
            .retain_context(realm)
            .unwrap();
        drop(context);
        Some(
            crate::engine::jobs::PendingJob::FinalizationRegistryCleanup {
                realm,
                callback,
                held_value: RawValue::Object(stale),
            },
        )
    } else {
        None
    };
    {
        let mut state = runtime.0.state.borrow_mut();
        state.release_jsvalue(JsValue::Object(stale)).unwrap();
        match case.as_str() {
            "deferred" => runtime
                .0
                .deferred_references
                .push_back(DeferredRefOp::Object(stale)),
            "kept" => {
                state
                    .kept_objects
                    .insert(crate::engine::heap::WeakCollectionKey::Object(stale));
            }
            "job" => {
                state.pending_jobs.push_back(job.unwrap());
            }
            _ => panic!("unknown final teardown subprocess"),
        }
        // Exception cleanup follows the job queue during final teardown.
        state.pending_exception = Some(RawValue::String(string));
    }
    assert!(!runtime.is_poisoned());
    assert_eq!(std::rc::Rc::strong_count(&runtime.0), 1);
    // No catch_unwind masks a destructor panic: final teardown must finish
    // without unwinding or aborting when an owned release reports an error.
    drop(runtime);
    assert!(runtime_weak.upgrade().is_none());
    // The only strong string owner was its heap node. This also fails under
    // release semantics if teardown ignores the first error and releases it,
    // or if StateStorage destroys the heap rather than abandoning its state.
    assert!(string_weak.upgrade().is_some());
}
