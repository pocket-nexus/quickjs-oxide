//! Host services: output capture, a fixed clock and seed, an in-memory file
//! system, and promise rejection events. Backs apps/web/docs/host.md.
//!
//! ```sh
//! cargo run --quiet --example host_services
//! ```

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use quickjs_oxide::engine::api::core::{
    CoreValue, Host, JsString, QjsHelpers, RejectionEvent, Runtime, RuntimeOptions, Stream, Value,
};

// docs:start capture-output
/// A host that keeps everything `print` and `console.log` write.
#[derive(Clone, Default)]
struct CaptureHost {
    output: Rc<RefCell<Vec<u8>>>,
}

impl Host for CaptureHost {
    fn write(&mut self, stream: Stream, text: &[u8]) {
        if stream == Stream::Stdout {
            self.output.borrow_mut().extend_from_slice(text);
        }
    }
}

fn capture_output() {
    let host = CaptureHost::default();
    let output = Rc::clone(&host.output);
    let runtime = Runtime::new_with_host_services(host);
    let context = runtime.context();
    let before = context.eval("typeof print + ' ' + typeof console").unwrap();
    assert_eq!(
        before,
        Value::String(JsString::from_static("undefined undefined"))
    );
    context.install_qjs_helpers(&QjsHelpers::default()).unwrap();

    context
        .eval("print('hello', 1 + 2); console.log('done')")
        .unwrap();

    assert_eq!(&*output.borrow(), b"hello 3\ndone\n");
}
// docs:end capture-output

// docs:start fixed-clock
/// A host with a fixed clock, a fixed time zone and a fixed random seed:
/// every run of a script sees the same `Date` and `Math.random` values.
struct FixedHost;

impl Host for FixedHost {
    fn now_ms(&mut self) -> f64 {
        1_767_225_600_000.0 // 2026-01-01T00:00:00Z
    }

    fn local_tz_offset_minutes(&mut self, _utc_ms: f64) -> i32 {
        -60 // UTC+1: getTimezoneOffset() is UTC minus local time
    }

    fn random_seed(&mut self) -> u64 {
        42
    }
}

fn fixed_clock() {
    let script = "[new Date().toISOString(), new Date().getTimezoneOffset(),
                   new Date().getHours(), Math.random()].join(' ')";
    let run = || {
        let runtime = Runtime::with_host(FixedHost, RuntimeOptions::default());
        let context = runtime.context();
        let Value::String(text) = context.eval(script).unwrap() else {
            panic!("expected a string");
        };
        text.to_utf8_lossy()
    };

    let first = run();
    assert!(first.starts_with("2026-01-01T00:00:00.000Z -60 1 0."));
    assert_eq!(first, run());
}
// docs:end fixed-clock

// docs:start read-file
/// A host whose `load` and `read` see an in-memory file system.
struct MemoryFiles {
    files: HashMap<String, String>,
}

impl Host for MemoryFiles {
    fn read_file(&mut self, path: &str) -> Option<String> {
        self.files.get(path).cloned()
    }
}

fn read_file() {
    let files = HashMap::from([
        ("lib.js".to_owned(), "var greeting = 'hi';".to_owned()),
        ("data.txt".to_owned(), "line one".to_owned()),
    ]);
    let runtime = Runtime::new_with_host_services(MemoryFiles { files });
    let context = runtime.context();
    context
        .install_qjs_helpers(&QjsHelpers {
            std: true,
            script_args: Some(vec!["main.js".to_owned()]),
        })
        .unwrap();

    let value = context
        .eval("load('lib.js'); greeting + ', ' + read('data.txt') + ', ' + scriptArgs[0]")
        .unwrap();
    let Value::String(text) = value else {
        panic!("expected a string")
    };
    assert_eq!(text.to_utf8_lossy(), "hi, line one, main.js");

    // A path the host does not know is a ReferenceError.
    let value = context
        .eval("try { load('missing.js') } catch (e) { e.name }")
        .unwrap();
    let Value::String(text) = value else {
        panic!("expected a string")
    };
    assert_eq!(text.to_utf8_lossy(), "ReferenceError");
}
// docs:end read-file

// docs:start rejection-events
/// A host that records the rejection tracker's events.
#[derive(Clone, Default)]
struct RejectionLog {
    events: Rc<RefCell<Vec<RejectionEvent>>>,
}

impl Host for RejectionLog {
    fn promise_rejection(&mut self, event: RejectionEvent, _promise: CoreValue) {
        self.events.borrow_mut().push(event);
    }
}

fn rejection_events() {
    let host = RejectionLog::default();
    let events = Rc::clone(&host.events);
    let runtime = Runtime::new_with_host_services(host);
    let context = runtime.context();

    context
        .eval("globalThis.p = Promise.reject(new Error('late'))")
        .unwrap();
    context.eval("p.catch(() => {})").unwrap();
    runtime.run_jobs().unwrap();

    assert_eq!(
        *events.borrow(),
        [RejectionEvent::Rejected, RejectionEvent::Handled]
    );
}
// docs:end rejection-events

fn native_system_host() {
    // docs:start system-host
    let runtime = Runtime::new_with_host_services(quickjs_oxide_host::SystemHost::default());
    let context = runtime.context();
    let offset = context.eval("new Date(0).getTimezoneOffset()").unwrap();
    // docs:end system-host
    assert!(matches!(offset, Value::Int(_)));
}

fn main() {
    capture_output();
    fixed_clock();
    read_file();
    rejection_events();
    native_system_host();
    println!("host: ok");
}
