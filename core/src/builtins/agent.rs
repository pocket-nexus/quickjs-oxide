//! `$262.agent`: Test262's multi-agent host, as QuickJS's `run-test262`
//! implements it. The embedder supplies the agent execution capability; the
//! core marshals shared buffers, reports and rooted broadcast callbacks.

use alloc::boxed::Box;
use alloc::string::String;
use alloc::sync::Arc;

use super::{Native, define, plain_object};
use crate::heap::HeapError;
use crate::heap::buffer::{Buffer, SharedBytes, Store};
use crate::heap::object::{InternalData, InternalTag, ObjectKind};
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

/// Agent execution and communication supplied by the embedder.
pub trait AgentHost: Send + Sync {
    fn start(self: Arc<Self>, source: String, runner: ScriptRunner, compile: Option<CompileFn>);
    fn join(&self);
    fn broadcast(&self, message: Message);
    fn take_report(&self) -> Option<String>;
    fn receive(&self, index: usize) -> Message;
    fn report(&self, text: String);
}

/// Creates an agent host only when the runtime first uses its agent services.
pub type AgentHostFactory = fn() -> Arc<dyn AgentHost>;

/// The agent side of a runtime: its position and rooted callback.
pub struct AgentSelf {
    index: usize,
    /// The `receiveBroadcast` callback (a fixed root), until called.
    callback: Option<Value>,
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

fn agent_host(services: &mut crate::vm::Services) -> Option<&Arc<dyn AgentHost>> {
    if services.agent_host.is_none() {
        let factory = services.agent_host_factory?;
        services.agent_host = Some(factory());
    }
    services.agent_host.as_ref()
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
    let Some(host) = agent_host(cx.host).cloned() else {
        return Err(error::type_error(
            cx,
            "agents are not supported by this host",
        ));
    };
    host.start(source, runner, cx.host.compile);
    Ok(Value::UNDEFINED)
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
    if let Some(host) = agent_host(cx.host) {
        host.broadcast(Message {
            bytes: shared.0,
            max: shared.1,
            value,
        });
    }
    Ok(Value::UNDEFINED)
}

/// `$262.agent.getReport()`: the oldest report, or `null`.
pub(super) fn get_report(cx: &mut Cx<'_>, _site: CallSite) -> Result<Value, Throw> {
    let report = agent_host(cx.host).and_then(|host| host.take_report());
    match report {
        Some(text) => super::string_text(cx, &text),
        None => Ok(Value::NULL),
    }
}

pub(super) fn sleep(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let time = site.arg(cx, 0);
    let time = ops::to_number(cx, time)?;
    cx.host.hooks.sleep_ms(time);
    Ok(Value::UNDEFINED)
}

pub(super) fn monotonic_now(cx: &mut Cx<'_>, _site: CallSite) -> Result<Value, Throw> {
    Ok(Value::number(cx.host.hooks.performance_now_ms().floor()))
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
    if cx.host.agent.is_some() {
        if let Some(host) = agent_host(cx.host) {
            host.report(text);
        }
    }
    Ok(Value::UNDEFINED)
}

pub(super) fn leaving(_cx: &mut Cx<'_>, _site: CallSite) -> Result<Value, Throw> {
    Ok(Value::UNDEFINED)
}

impl Runtime {
    /// Supplies execution and communication for `$262.agent`.
    pub fn set_agent_host(&mut self, host: Arc<dyn AgentHost>) {
        self.host.agent_host = Some(host);
        self.host.agent_host_factory = None;
    }

    /// Supplies an agent host lazily, retaining the no-agent allocation path.
    pub fn set_agent_host_factory(&mut self, factory: AgentHostFactory) {
        self.host.agent_host = None;
        self.host.agent_host_factory = Some(factory);
    }

    /// Lets `$262.agent.start` run agents' scripts through `runner`.
    pub fn set_script_runner(&mut self, runner: ScriptRunner) {
        self.host.script_runner = Some(runner);
    }

    /// Waits for every agent this runtime started.
    pub fn join_agents(&mut self) {
        if let Some(host) = &self.host.agent_host {
            host.join();
        }
    }

    /// Defines an agent's `$262` (only `agent`).
    pub fn install_agent_host(
        &mut self,
        host: Arc<dyn AgentHost>,
        index: usize,
    ) -> Result<(), HeapError> {
        self.set_agent_host(host);
        self.host.agent = Some(AgentSelf {
            index,
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

    /// Takes the rooted callback and the agent's broadcast position.
    pub fn take_agent_callback(&mut self) -> Option<(usize, Value)> {
        self.host.agent.as_mut().and_then(|agent| {
            agent
                .callback
                .take()
                .map(|callback| (agent.index, callback))
        })
    }

    /// Calls a broadcast callback with a buffer wrapping the shared bytes.
    pub fn call_agent_callback(
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
