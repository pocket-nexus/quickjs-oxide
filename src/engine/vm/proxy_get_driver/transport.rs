//! Diagnostic Query lifetime accounting; absent from ordinary release builds.
use super::Finish;
use crate::engine::api::profiling::record_owned_execution_event;

#[derive(Default)]
enum State {
    #[default]
    Untracked,
    Acquired,
    Effect,
    Completed,
}

#[derive(Default)]
enum Family {
    Read,
    Write,
    Call,
    #[default]
    Other,
}

#[derive(Default)]
pub(super) struct QueryTransport {
    family: Family,
    state: State,
}

impl QueryTransport {
    pub(super) fn acquired(finish: &Finish) -> Self {
        let family = match finish {
            Finish::ComputedRead(_) | Finish::PropertyKeyValue { .. } | Finish::PropertyRead(_) => {
                Family::Read
            }
            Finish::ResidentWrite { .. } | Finish::Write { .. } => Family::Write,
            Finish::Call { .. } | Finish::ResidentCall { .. } | Finish::VmCall(_) => Family::Call,
            _ => Family::Other,
        };
        record_owned_execution_event("query_transport.acquired");
        Self {
            family,
            state: State::Acquired,
        }
    }

    pub(super) fn effect(&mut self, reason: &'static str) {
        if matches!(self.state, State::Acquired | State::Effect) {
            record_owned_execution_event(reason);
            self.state = State::Effect;
        }
    }

    pub(super) fn complete(&mut self) {
        match self.state {
            State::Acquired => {
                record_owned_execution_event("query_transport.completed_without_effect");
                record_owned_execution_event(match self.family {
                    Family::Read => "query_transport.completed_without_effect.read",
                    Family::Write => "query_transport.completed_without_effect.write",
                    Family::Call => "query_transport.completed_without_effect.call",
                    Family::Other => "query_transport.completed_without_effect.other",
                });
            }
            State::Effect => record_owned_execution_event("query_transport.completed_after_effect"),
            State::Untracked | State::Completed => return,
        }
        self.state = State::Completed;
    }
}

impl super::Query {
    pub(in crate::engine::vm) fn record_transport_effect(&mut self, reason: &'static str) {
        self.transport.effect(reason);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::api::profiling::CostProfile;

    #[test]
    fn only_actual_effects_exempt_a_completed_query() {
        let profile = CostProfile::start();
        let mut synchronous = QueryTransport::acquired(&Finish::PropertyRead(0));
        synchronous.complete();
        synchronous.complete();
        let mut callback = QueryTransport::acquired(&Finish::PropertyRead(0));
        callback.effect("query_transport.effect.js_child");
        callback.complete();
        let mut boundary = QueryTransport::acquired(&Finish::PropertyRead(0));
        boundary.effect("query_transport.effect.state_boundary");
        boundary.complete();
        let _abandoned = QueryTransport::acquired(&Finish::PropertyRead(0));
        QueryTransport::default().complete();
        let events = profile.snapshot().owned_execution_events;
        assert_eq!(events["query_transport.acquired"], 4);
        assert_eq!(events["query_transport.completed_without_effect"], 1);
        assert_eq!(events["query_transport.completed_without_effect.read"], 1);
        assert_eq!(events["query_transport.completed_after_effect"], 2);
    }

    #[test]
    fn current_synchronous_heap_element_reads_expose_unused_transport() {
        let runtime = crate::engine::api::Runtime::new();
        let mut context = runtime.new_context().unwrap();
        let profile = CostProfile::start();
        assert_eq!(
            context
                .eval("let o={};let a=[o];let n=0;for(let i=0;i<8;i++){if(a[0]===o)n++}n")
                .unwrap(),
            crate::engine::value::Value::Int(8)
        );
        let events = profile.snapshot().owned_execution_events;
        assert!(
            events
                .get("query_transport.completed_without_effect.read")
                .copied()
                .unwrap_or(0)
                >= 8
        );
    }

    #[test]
    fn real_getter_and_proxy_waits_have_effects() {
        let runtime = crate::engine::api::Runtime::new();
        let mut context = runtime.new_context().unwrap();
        let profile = CostProfile::start();
        assert_eq!(context.eval("let a={get x(){return {v:3}}};let p=new Proxy(a,{get(t,k,r){return Reflect.get(t,k,r)}});a['x'].v+p['x'].v").unwrap(),
                   crate::engine::value::Value::Int(6));
        let events = profile.snapshot().owned_execution_events;
        assert!(
            events
                .get("query_transport.effect.js_child")
                .copied()
                .unwrap_or(0)
                > 0
        );
        assert!(
            events
                .get("query_transport.effect.state_boundary")
                .copied()
                .unwrap_or(0)
                > 0
        );
        assert!(
            events
                .get("query_transport.completed_after_effect")
                .copied()
                .unwrap_or(0)
                > 0
        );
    }
}
