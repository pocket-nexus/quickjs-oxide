//! Attribute internal execution separately from external roots and admission.
//! This is diagnostic state only; it grants no heap access or ownership.

use std::cell::Cell;

thread_local! {
    static IN_CORE: Cell<bool> = const { Cell::new(false) };
}

pub(crate) struct CoreExecutionScope(bool);

impl CoreExecutionScope {
    pub(crate) fn enter() -> Self {
        Self(IN_CORE.with(|scope| scope.replace(true)))
    }

    pub(crate) fn outside() -> Self {
        Self(IN_CORE.with(|scope| scope.replace(false)))
    }
}

impl Drop for CoreExecutionScope {
    fn drop(&mut self) {
        IN_CORE.with(|scope| scope.set(self.0));
    }
}

pub(crate) fn record_runtime_event(total: &'static str, internal: &'static str) {
    super::record_owned_execution_event(total);
    if IN_CORE.with(Cell::get) {
        super::record_owned_execution_event(internal);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::api::profiling::CostProfile;

    #[test]
    fn external_callbacks_and_unwind_restore_the_previous_origin() {
        let profile = CostProfile::start();
        record_runtime_event("all", "internal");
        {
            let _core = CoreExecutionScope::enter();
            record_runtime_event("all", "internal");
            let result = std::panic::catch_unwind(|| {
                let _external = CoreExecutionScope::outside();
                record_runtime_event("all", "internal");
                let _nested = CoreExecutionScope::enter();
                record_runtime_event("all", "internal");
                panic!("origin restoration probe");
            });
            assert!(result.is_err());
            record_runtime_event("all", "internal");
        }
        record_runtime_event("all", "internal");
        let events = profile.snapshot().owned_execution_events;
        assert_eq!(events["all"], 6);
        assert_eq!(events["internal"], 3);
    }
}
