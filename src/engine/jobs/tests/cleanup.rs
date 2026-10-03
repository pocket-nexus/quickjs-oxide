use super::*;

#[test]
fn failed_owned_job_cleanup_stops_and_quarantines() {
    for case in ["finish", "drop", "prepared", "discard"] {
        let runtime = Runtime::new();
        let context = runtime.new_context().unwrap();
        let realm = context.realm_id();
        let callback = runtime.new_object(None).unwrap().into_handle();
        let stale = runtime.new_object(None).unwrap().into_handle();
        let string = JsString::from_static("later prepared-job owner");
        let string_weak = string.downgrade();
        let JsValue::String(string) = runtime.into_jsvalue(Value::String(string)).unwrap() else {
            panic!("owned string");
        };
        let multiple_jobs = matches!(case, "prepared" | "discard");
        let (callback_count, realm_count) = {
            let mut state = runtime.0.state.borrow_mut();
            state.release_jsvalue(JsValue::Object(stale)).unwrap();
            state.heap.retain_context(realm).unwrap();
            if multiple_jobs {
                state.heap.retain_context(realm).unwrap();
                state.heap.retain_object(callback).unwrap();
            }
            (
                state.heap.object_strong_count(callback).unwrap(),
                state.heap.context_strong_count(realm).unwrap(),
            )
        };
        let first = PendingJob::FinalizationRegistryCleanup {
            realm,
            callback,
            held_value: RawValue::Object(stale),
        };
        // Job roots are released backwards. The broken held-value edge comes
        // before the still-valid callback and realm, and before a later job.
        match case {
            "finish" => assert!(matches!(
                PendingJobRootGuard::new(&runtime, first).finish(),
                Err(RuntimeError::Heap(HeapError::Stale { .. }))
            )),
            "drop" => drop(PendingJobRootGuard::new(&runtime, first)),
            "prepared" | "discard" => {
                let later = PendingJob::FinalizationRegistryCleanup {
                    realm,
                    callback,
                    held_value: RawValue::String(string),
                };
                let jobs = vec![first, later];
                if case == "prepared" {
                    drop(PreparedJobs::new(&runtime, jobs));
                } else {
                    assert!(matches!(
                        runtime.discard_prepared_jobs(jobs),
                        Err(RuntimeError::Heap(HeapError::Stale { .. }))
                    ));
                }
            }
            _ => unreachable!(),
        }
        assert!(runtime.is_poisoned(), "job cleanup case {case}");
        let state = runtime.0.state.borrow();
        assert_eq!(state.heap.object_strong_count(callback), Ok(callback_count));
        assert_eq!(state.heap.context_strong_count(realm), Ok(realm_count));
        drop(state);
        assert!(string_weak.upgrade().is_some());
        assert!(matches!(
            runtime.memory_snapshot(),
            Err(RuntimeError::Poisoned)
        ));
    }
}
