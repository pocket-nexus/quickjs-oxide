//! `$262.agent`: Test262's multi-agent host object, and the host capability
//! behind it.
//!
//! The natives here marshal JavaScript into plain data — a source string, the
//! bytes of a `SharedArrayBuffer`, a report, a value — and hand it to the
//! `AgentHost` the host installed. The agents themselves are threads, a mutex
//! and condition variables, which is why none of that is in this crate: a
//! runtime with no `AgentHost` has no agents, and `$262.agent.start` says so.
//!
//! A broadcast is the shared bytes of a buffer (an `Arc`, so no heap crosses
//! threads), the buffer's maximum length, and the value. Reports are strings in
//! a queue.

use alloc::boxed::Box;

use alloc::string::String;
use alloc::sync::Arc;

use super::{Native, define, plain_object};
use crate::float;
use crate::heap::HeapError;
use crate::heap::buffer::SharedBytes;
use crate::runtime::{Intrinsic, Runtime};
use crate::shape::PropertyFlags;
use crate::value::Value;
use crate::vm::compile::CompileFn;
use crate::vm::native::CallSite;
use crate::vm::{Cx, Throw, error, ops, typed};

/// Runs a script's source in a runtime (the embedder compiles it), then
/// its jobs; the error is the uncaught exception's text.
pub type ScriptRunner = fn(&mut Runtime, &str) -> Result<(), String>;

/// A broadcast: the shared bytes, the buffer's maximum length, the value.
#[derive(Clone)]
pub struct Message {
    pub bytes: Arc<SharedBytes>,
    pub max: Option<usize>,
    pub value: f64,
}

/// The agent machinery a host with threads brings.
///
/// The hub is one object shared by a main agent and the agents it started;
/// each runtime holds it beside its own index, so the same trait serves both
/// sides. Every method is called with the runtime's borrow released: the
/// caller clones the `Arc` before it enters.
pub trait AgentHost: Send + Sync {
    /// Starts an agent running `source`. False when the host can not.
    fn start(
        self: Arc<Self>,
        source: &str,
        runner: ScriptRunner,
        compile: Option<CompileFn>,
    ) -> bool;

    /// Waits for every agent this hub started.
    fn join(&self);

    /// `$262.agent.broadcast`: hands every agent the message and blocks until
    /// each took it.
    fn broadcast(&self, message: Message);

    /// `$262.agent.report` from agent `index`.
    fn report(&self, index: usize, text: String);

    /// `$262.agent.getReport`: the oldest report, if any.
    fn get_report(&self) -> Option<String>;

    /// The next broadcast for agent `index`, blocking until there is one.
    fn receive(&self, index: usize) -> Message;
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

/// `$262.agent.start(source)`.
pub(super) fn start(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let source = site.arg(cx, 0);
    let source = ops::to_string(cx, source)?;
    let source = cx.heap.string_text(source.as_string().unwrap());
    let runner = cx.host.script_runner;
    let hub = cx.host.agents.clone();
    let compile = cx.host.compile;
    let started = match (runner, hub) {
        (Some(runner), Some(hub)) => hub.start(&source, runner, compile),
        _ => false,
    };
    if started {
        Ok(Value::UNDEFINED)
    } else {
        Err(error::type_error(
            cx,
            "agents are not supported by this host",
        ))
    }
}

/// `$262.agent.broadcast(sab, value)`: blocks until every agent took it.
pub(super) fn broadcast(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let buffer = site.arg(cx, 0);
    let shared = match buffer.as_object() {
        Some(object)
            if cx.heap.internal_tag(object)
                == crate::heap::object::InternalTag::SharedArrayBuffer =>
        {
            let record = typed::buffer(cx.heap, object);
            match &record.store {
                crate::heap::buffer::Store::Shared(bytes) => (bytes.clone(), record.max),
                crate::heap::buffer::Store::Owned(_) => unreachable!("a shared buffer"),
            }
        }
        _ => return Err(error::object_expected(cx, "ArrayBuffer")),
    };
    let value = site.arg(cx, 1);
    let value = ops::to_number(cx, value)?;
    let value = f64::from(crate::vm::number::to_int32(value));
    let Some(hub) = cx.host.agents.clone() else {
        return Err(error::type_error(
            cx,
            "agents are not supported by this host",
        ));
    };
    hub.broadcast(Message {
        bytes: shared.0,
        max: shared.1,
        value,
    });
    Ok(Value::UNDEFINED)
}

/// `$262.agent.getReport()`: the oldest report, or `null`.
pub(super) fn get_report(cx: &mut Cx<'_>, _site: CallSite) -> Result<Value, Throw> {
    let report = cx.host.agents.as_ref().and_then(|hub| hub.get_report());
    match report {
        Some(text) => super::string_text(cx, &text),
        None => Ok(Value::NULL),
    }
}

pub(super) fn sleep(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let time = site.arg(cx, 0);
    let time = ops::to_number(cx, time)?;
    if time > 0.0 {
        cx.host.hooks.sleep_ms(time);
    }
    Ok(Value::UNDEFINED)
}

pub(super) fn monotonic_now(cx: &mut Cx<'_>, _site: CallSite) -> Result<Value, Throw> {
    Ok(Value::number(float::floor(cx.host.monotonic_ms())))
}

/// `$262.agent.receiveBroadcast(callback)` in an agent.
pub(super) fn receive_broadcast(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let callback = site.arg(cx, 0);
    if !ops::is_callable(cx.heap, callback) {
        return Err(error::type_error(cx, "expecting function"));
    }
    cx.heap.add_fixed_root(callback);
    cx.host.agent_callback = Some(callback);
    Ok(Value::UNDEFINED)
}

/// `$262.agent.report(value)` in an agent.
pub(super) fn report(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let value = site.arg(cx, 0);
    let text = ops::to_string(cx, value)?;
    let text = cx.heap.string_text(text.as_string().unwrap());
    if let (Some(hub), Some(index)) = (cx.host.agents.clone(), cx.host.agent_index) {
        hub.report(index, text);
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

    /// Installs the agents this runtime coordinates as a main agent.
    pub fn set_agent_host(&mut self, hub: Arc<dyn AgentHost>) {
        self.host.agents = Some(hub);
        self.host.agent_index = None;
    }

    /// Waits for every agent this runtime started.
    pub fn join_agents(&mut self) {
        if let Some(hub) = self.host.agents.clone() {
            hub.join();
        }
    }

    /// Makes this runtime agent `index` of `hub`, and defines its `$262`.
    pub fn install_agent_host(
        &mut self,
        hub: Arc<dyn AgentHost>,
        index: usize,
    ) -> Result<(), HeapError> {
        self.host.agents = Some(hub);
        self.host.agent_index = Some(index);
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

    /// The `receiveBroadcast` callback an agent registered, once.
    pub fn take_agent_callback(&mut self) -> Option<Value> {
        self.host.agent_callback.take()
    }

    /// Calls a waiting agent's callback with a broadcast it received.
    pub fn call_agent_callback(&mut self, callback: Value, message: Message) -> Result<(), Throw> {
        let cx = self.cx();
        let prototype = cx.realm.intrinsic(Intrinsic::SharedArrayBufferPrototype);
        let buffer = crate::heap::buffer::Buffer {
            store: crate::heap::buffer::Store::Shared(message.bytes),
            detached: false,
            max: message.max,
        };
        let object = cx
            .heap
            .alloc_internal(
                crate::heap::object::ObjectKind::Builtin,
                crate::heap::object::InternalTag::SharedArrayBuffer,
                Some(prototype),
                0,
                &[],
                crate::heap::object::InternalData::Buffer(Box::new(buffer)),
                0,
            )
            .map_err(|error| error::heap_error(&cx, error))?;
        let handle = self.hold(Value::object(object));
        let result = self.call(
            callback,
            Value::UNDEFINED,
            &[Value::object(object), Value::number(message.value)],
        );
        self.release(handle);
        result.map(drop)
    }
}
