//! The agents a Test262 run starts: one thread each, a queue of broadcasts
//! behind one mutex, reports in one queue, and the clock an agent's runtime
//! needs to sleep.
//!
//! The core owns the `$262.agent` object and marshals its arguments; this is
//! the machinery behind it, and it is here because it needs threads.

use std::collections::VecDeque;
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use oxide_core::Runtime;
use oxide_core::builtins::agent::{AgentHost, Message, ScriptRunner};
use oxide_core::heap::gc::HeapLimits;
use oxide_core::host::{Host, Monotonic};
use oxide_core::vm::compile::CompileFn;

#[derive(Default)]
struct HubState {
    /// Each agent's pending broadcast.
    pending: Vec<Option<Message>>,
    reports: VecDeque<String>,
    threads: Vec<JoinHandle<()>>,
}

/// What the main agent and its agents share.
#[derive(Default)]
pub(crate) struct ProcessAgents {
    state: Mutex<HubState>,
    changed: Condvar,
}

impl ProcessAgents {
    pub(crate) fn new() -> Arc<ProcessAgents> {
        Arc::new(ProcessAgents::default())
    }

    fn lock(&self) -> MutexGuard<'_, HubState> {
        // The queue stays structurally valid if an agent panics, so a poisoned
        // gate is recovered rather than disabling every later broadcast.
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

impl AgentHost for ProcessAgents {
    fn start(
        self: Arc<Self>,
        source: &str,
        runner: ScriptRunner,
        compile: Option<CompileFn>,
    ) -> bool {
        let index = {
            let mut state = self.lock();
            state.pending.push(None);
            state.pending.len() - 1
        };
        let hub = self.clone();
        let source = source.to_owned();
        let thread = std::thread::spawn(move || run_agent(hub, index, &source, runner, compile));
        self.lock().threads.push(thread);
        true
    }

    fn join(&self) {
        let threads = core::mem::take(&mut self.lock().threads);
        for thread in threads {
            let _ = thread.join();
        }
    }

    fn broadcast(&self, message: Message) {
        let mut state = self.lock();
        for slot in &mut state.pending {
            *slot = Some(message.clone());
        }
        self.changed.notify_all();
        while state.pending.iter().any(Option::is_some) {
            state = self
                .changed
                .wait(state)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
        }
    }

    fn report(&self, _index: usize, text: String) {
        // QuickJS keeps one report queue, not one per agent.
        self.lock().reports.push_back(text);
    }

    fn get_report(&self) -> Option<String> {
        self.lock().reports.pop_front()
    }

    fn receive(&self, index: usize) -> Message {
        let mut state = self.lock();
        loop {
            if let Some(message) = state.pending[index].take() {
                self.changed.notify_all();
                return message;
            }
            state = self
                .changed
                .wait(state)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
        }
    }
}

/// An agent thread: the script, its jobs, then one broadcast if it asked
/// for one (QuickJS's `agent_start`).
fn run_agent(
    hub: Arc<ProcessAgents>,
    index: usize,
    source: &str,
    runner: ScriptRunner,
    compile: Option<CompileFn>,
) {
    let Ok(mut runtime) = Runtime::new(HeapLimits::default()) else {
        return;
    };
    runtime.set_host(Box::new(AgentClock));
    if let Some(compile) = compile {
        runtime.set_compiler(compile);
    }
    let shared: Arc<dyn AgentHost> = hub.clone();
    if runtime.install_print().is_err() || runtime.install_agent_host(shared, index).is_err() {
        return;
    }
    if let Err(message) = runner(&mut runtime, source) {
        eprintln!("{message}");
        return;
    }
    loop {
        if runtime.run_jobs().is_err() {
            return;
        }
        let Some(callback) = runtime.take_agent_callback() else {
            return;
        };
        let message = hub.receive(index);
        if runtime.call_agent_callback(callback, message).is_err() {
            return;
        }
    }
}

/// An agent's clock. Its runtime has no streams and no files — the core
/// discards what it prints, as it always has — so the reading and the sleep
/// are the whole host.
struct AgentClock;

impl Host for AgentClock {
    fn monotonic(&mut self) -> Monotonic {
        Monotonic::new(agent_monotonic_ns)
    }

    fn sleep_ms(&mut self, milliseconds: f64) {
        if milliseconds > 0.0 {
            std::thread::sleep(Duration::from_secs_f64(milliseconds / 1000.0));
        }
    }
}

/// Nanoseconds from the process's first use, for an agent's `performance.now`.
fn agent_monotonic_ns() -> u64 {
    static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_nanos() as u64
}
