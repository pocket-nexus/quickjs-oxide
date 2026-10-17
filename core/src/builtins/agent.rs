//! `$262.agent` (phase 4, section 6.1 row 12): Test262's multi-agent host,
//! as QuickJS's `run-test262` implements it. Each agent is a thread with
//! its own runtime running a script through the embedder's
//! [`ScriptRunner`]; a broadcast hands every agent the shared bytes of a
//! `SharedArrayBuffer` (an `Arc`, so no heap crosses threads) and waits
//! until each has taken them; reports are strings in a shared queue.

use std::collections::VecDeque;
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use super::{Native, define, plain_object};
use crate::heap::HeapError;
use crate::heap::buffer::{Buffer, SharedBytes, Store};
use crate::heap::gc::HeapLimits;
use crate::heap::object::{InternalData, InternalTag, ObjectKind};
use crate::runtime::{Intrinsic, Runtime};
use crate::shape::PropertyFlags;
use crate::value::Value;
use crate::vm::native::CallSite;
use crate::vm::{Cx, Throw, error, ops, typed};

/// Runs a script's source in a runtime (the embedder compiles it), then
/// its jobs; the error is the uncaught exception's text.
pub type ScriptRunner = fn(&mut Runtime, &str) -> Result<(), String>;

/// A broadcast: the shared bytes, the buffer's maximum length, the value.
#[derive(Clone)]
struct Message {
    bytes: Arc<SharedBytes>,
    max: Option<usize>,
    value: f64,
}

#[derive(Default)]
struct HubState {
    /// Each agent's pending broadcast.
    pending: Vec<Option<Message>>,
    reports: VecDeque<String>,
    threads: Vec<JoinHandle<()>>,
}

/// What the main agent and its agents share.
#[derive(Default)]
pub struct AgentHub {
    state: Mutex<HubState>,
    changed: Condvar,
}

impl AgentHub {
    fn lock(&self) -> MutexGuard<'_, HubState> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Joins every agent thread.
    pub fn join(&self) {
        let threads = std::mem::take(&mut self.lock().threads);
        for thread in threads {
            let _ = thread.join();
        }
    }
}

/// The agent side of a runtime: its hub and position.
pub struct AgentSelf {
    hub: Arc<AgentHub>,
    /// The `receiveBroadcast` callback (a fixed root), until called.
    callback: Option<Value>,
}

static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();

pub(super) fn monotonic_ms() -> f64 {
    START.get_or_init(Instant::now).elapsed().as_secs_f64() * 1000.0
}

/// Defines `$262.agent` on `host` for the main agent (`main`) or an agent.
pub(crate) fn install(cx: &mut Cx<'_>, host: crate::ObjectId, main: bool) -> Result<(), HeapError> {
    let prototype = cx.realm.intrinsic(Intrinsic::ObjectPrototype);
    let agent = plain_object(cx, Some(prototype), 8)?;
    let all = PropertyFlags::DEFAULT;
    let natives: &[Native] = if main {
        &[
            Native::AgentStart,
            Native::AgentBroadcast,
            Native::AgentGetReport,
            Native::AgentSleep,
            Native::AgentMonotonicNow,
        ]
    } else {
        &[
            Native::AgentReceiveBroadcast,
            Native::AgentReport,
            Native::AgentLeaving,
            Native::AgentSleep,
            Native::AgentMonotonicNow,
        ]
    };
    for &native in natives {
        let function = super::native_function(cx, native)?;
        define(cx, agent, native.js_name(), Value::object(function), all)?;
    }
    define(cx, host, "agent", Value::object(agent), all)
}

fn hub(cx: &mut Cx<'_>) -> Arc<AgentHub> {
    cx.host
        .agent_hub
        .get_or_insert_with(Default::default)
        .clone()
}

/// `$262.agent.start(source)`.
pub(super) fn start(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let source = site.arg(cx, 0);
    let source = ops::to_string(cx, source)?;
    let source = cx.heap.string_text(source.as_string().unwrap());
    let Some(runner) = cx.host.script_runner else {
        return Err(error::type_error(
            cx,
            "agents are not supported by this host",
        ));
    };
    let hub = hub(cx);
    let index = {
        let mut state = hub.lock();
        state.pending.push(None);
        state.pending.len() - 1
    };
    let agent_hub = hub.clone();
    let compile = cx.host.compile;
    let thread = std::thread::spawn(move || run_agent(agent_hub, index, &source, runner, compile));
    hub.lock().threads.push(thread);
    Ok(Value::UNDEFINED)
}

/// An agent thread: the script, its jobs, then one broadcast if it asked
/// for one (QuickJS's `agent_start`).
fn run_agent(
    hub: Arc<AgentHub>,
    index: usize,
    source: &str,
    runner: ScriptRunner,
    compile: Option<crate::vm::compile::CompileFn>,
) {
    let Ok(mut runtime) = Runtime::new(HeapLimits::default()) else {
        return;
    };
    runtime.host.compile = compile;
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
        let Some(callback) = runtime.take_agent_callback() else {
            return;
        };
        let message = {
            let mut state = hub.lock();
            loop {
                if let Some(message) = state.pending[index].take() {
                    hub.changed.notify_all();
                    break message;
                }
                state = hub
                    .changed
                    .wait(state)
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
            }
        };
        if runtime
            .call_agent_callback(callback, message.bytes, message.max, message.value)
            .is_err()
        {
            return;
        }
    }
}

/// `$262.agent.broadcast(sab, value)`: blocks until every agent took it.
pub(super) fn broadcast(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let buffer = site.arg(cx, 0);
    let shared = match buffer.as_object() {
        Some(object) if cx.heap.internal_tag(object) == InternalTag::SharedArrayBuffer => {
            let record = typed::buffer(cx.heap, object);
            match &record.store {
                Store::Shared(bytes) => (bytes.clone(), record.max),
                Store::Owned(_) => unreachable!("a shared buffer"),
            }
        }
        _ => return Err(error::object_expected(cx, "ArrayBuffer")),
    };
    let value = site.arg(cx, 1);
    let value = ops::to_number(cx, value)?;
    let value = f64::from(crate::vm::number::to_int32(value));
    let hub = hub(cx);
    let mut state = hub.lock();
    let message = Message {
        bytes: shared.0,
        max: shared.1,
        value,
    };
    for slot in &mut state.pending {
        *slot = Some(message.clone());
    }
    hub.changed.notify_all();
    while state.pending.iter().any(Option::is_some) {
        state = hub
            .changed
            .wait(state)
            .unwrap_or_else(std::sync::PoisonError::into_inner);
    }
    Ok(Value::UNDEFINED)
}

/// `$262.agent.getReport()`: the oldest report, or `null`.
pub(super) fn get_report(cx: &mut Cx<'_>, _site: CallSite) -> Result<Value, Throw> {
    let hub = hub(cx);
    let report = hub.lock().reports.pop_front();
    match report {
        Some(text) => super::string_text(cx, &text),
        None => Ok(Value::NULL),
    }
}

pub(super) fn sleep(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let time = site.arg(cx, 0);
    let time = ops::to_number(cx, time)?;
    if time > 0.0 {
        std::thread::sleep(Duration::from_secs_f64(time / 1000.0));
    }
    Ok(Value::UNDEFINED)
}

pub(super) fn monotonic_now(_cx: &mut Cx<'_>, _site: CallSite) -> Result<Value, Throw> {
    Ok(Value::number(monotonic_ms().floor()))
}

/// `$262.agent.receiveBroadcast(callback)` in an agent.
pub(super) fn receive_broadcast(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let callback = site.arg(cx, 0);
    if !ops::is_callable(cx.heap, callback) {
        return Err(error::type_error(cx, "expecting function"));
    }
    cx.heap.add_fixed_root(callback);
    if let Some(agent) = &mut cx.host.agent {
        agent.callback = Some(callback);
    }
    Ok(Value::UNDEFINED)
}

/// `$262.agent.report(value)` in an agent.
pub(super) fn report(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let value = site.arg(cx, 0);
    let text = ops::to_string(cx, value)?;
    let text = cx.heap.string_text(text.as_string().unwrap());
    if let Some(agent) = &cx.host.agent {
        agent.hub.lock().reports.push_back(text);
    }
    Ok(Value::UNDEFINED)
}

pub(super) fn leaving(_cx: &mut Cx<'_>, _site: CallSite) -> Result<Value, Throw> {
    Ok(Value::UNDEFINED)
}

impl Runtime {
    /// Lets `$262.agent.start` run agents' scripts through `runner`.
    pub fn set_script_runner(&mut self, runner: ScriptRunner) {
        self.host.script_runner = Some(runner);
    }

    /// Waits for every agent this runtime started.
    pub fn join_agents(&mut self) {
        if let Some(hub) = self.host.agent_hub.clone() {
            hub.join();
        }
    }

    /// Defines an agent's `$262` (only `agent`).
    fn install_agent_host(&mut self, hub: Arc<AgentHub>, index: usize) -> Result<(), HeapError> {
        let _ = index;
        self.host.agent = Some(AgentSelf {
            hub,
            callback: None,
        });
        let mut cx = self.cx();
        let mark = cx.heap.roots().mark();
        let result = (|| {
            let prototype = cx.realm.intrinsic(Intrinsic::ObjectPrototype);
            let host = plain_object(&mut cx, Some(prototype), 2)?;
            install(&mut cx, host, false)?;
            let global = cx.realm.global();
            define(
                &mut cx,
                global,
                "$262",
                Value::object(host),
                PropertyFlags::DEFAULT,
            )
        })();
        cx.heap.roots().truncate(mark);
        result
    }

    fn take_agent_callback(&mut self) -> Option<Value> {
        self.host
            .agent
            .as_mut()
            .and_then(|agent| agent.callback.take())
    }

    fn call_agent_callback(
        &mut self,
        callback: Value,
        bytes: Arc<SharedBytes>,
        max: Option<usize>,
        value: f64,
    ) -> Result<(), Throw> {
        let cx = self.cx();
        let prototype = cx.realm.intrinsic(Intrinsic::SharedArrayBufferPrototype);
        let buffer = Buffer {
            store: Store::Shared(bytes),
            detached: false,
            max,
        };
        let object = cx
            .heap
            .alloc_internal(
                ObjectKind::Builtin,
                InternalTag::SharedArrayBuffer,
                Some(prototype),
                0,
                &[],
                InternalData::Buffer(Box::new(buffer)),
                0,
            )
            .map_err(|error| error::heap_error(&cx, error))?;
        let handle = self.hold(Value::object(object));
        let result = self.call(
            callback,
            Value::UNDEFINED,
            &[Value::object(object), Value::number(value)],
        );
        self.release(handle);
        result.map(drop)
    }
}
