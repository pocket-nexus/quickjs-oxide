//! Process-backed execution and communication for Test262 agents.

use std::collections::VecDeque;
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread::JoinHandle;

use oxide_core::builtins::agent::{AgentHost, Message, ScriptRunner};
use oxide_core::vm::compile::CompileFn;
use oxide_core::{HeapLimits, Runtime};

#[derive(Default)]
struct HubState {
    /// Each agent's pending broadcast.
    pending: Vec<Option<Message>>,
    reports: VecDeque<String>,
    threads: Vec<JoinHandle<()>>,
}

/// What a Test262 main agent and its agents share.
#[derive(Default)]
pub(super) struct ProcessAgents {
    state: Mutex<HubState>,
    changed: Condvar,
}

impl ProcessAgents {
    fn lock(&self) -> MutexGuard<'_, HubState> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

impl AgentHost for ProcessAgents {
    fn start(self: Arc<Self>, source: String, runner: ScriptRunner, compile: Option<CompileFn>) {
        let index = {
            let mut state = self.lock();
            state.pending.push(None);
            state.pending.len() - 1
        };
        let hub = self.clone();
        let thread = std::thread::spawn(move || run_agent(hub, index, &source, runner, compile));
        self.lock().threads.push(thread);
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

    fn take_report(&self) -> Option<String> {
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

    fn report(&self, text: String) {
        self.lock().reports.push_back(text);
    }
}

/// A script, its jobs, then each broadcast callback it registers.
fn run_agent(
    hub: Arc<ProcessAgents>,
    index: usize,
    source: &str,
    runner: ScriptRunner,
    compile: Option<CompileFn>,
) {
    let Ok(mut runtime) = Runtime::with_host(
        HeapLimits::default(),
        Box::new(super::host::Hosted(super::host::SystemHost)),
    ) else {
        return;
    };
    runtime.set_waiter(super::waiter::ProcessWaiter::shared());
    if let Some(compile) = compile {
        runtime.set_compiler(compile);
    }
    if runtime.install_print().is_err() || runtime.install_agent_host(hub.clone(), index).is_err() {
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
        let Some((index, callback)) = runtime.take_agent_callback() else {
            return;
        };
        let message = hub.receive(index);
        if runtime
            .call_agent_callback(callback, message.bytes, message.max, message.value)
            .is_err()
        {
            return;
        }
    }
}
