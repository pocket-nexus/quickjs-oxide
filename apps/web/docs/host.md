# Host services

The engine reaches the outside world through one trait, `Host`. A runtime owns one host for its whole life. The host decides where output goes, what time it is, which time zone applies, how `Math.random` is seeded, what happens on a promise rejection, and how `load` and `read` find files.

## The Host trait

`Host` is defined in [`core/src/host.rs`](https://github.com/pocket-nexus/quickjs-oxide/blob/main/core/src/host.rs) and re-exported as `quickjs_oxide::engine::api::core::Host`. Every method has a default, so an implementation overrides only what it needs.

| Method | Used by | Default |
| --- | --- | --- |
| `write(&mut self, stream: Stream, text: &[u8])` | `print`, `console.log` | writes to the process's stdout or stderr and flushes |
| `now_ms(&mut self) -> f64` | `Date.now()`, `new Date()` | the system clock, in milliseconds since the Unix epoch |
| `local_tz_offset_minutes(&mut self, utc_ms: f64) -> i32` | local-time `Date` methods | `0` (UTC) |
| `random_seed(&mut self) -> u64` | the initial state of `Math.random` | derived from the system clock |
| `promise_rejection(&mut self, event: RejectionEvent, promise: CoreValue)` | the promise rejection tracker | ignores the event |
| `read_file(&mut self, path: &str) -> Option<String>` | `load` and `read` | reads the process's file system |

`local_tz_offset_minutes` returns UTC minus local time at the instant `utc_ms`, with the sign `Date.prototype.getTimezoneOffset` uses: a zone east of UTC gives a negative number. The engine asks for every instant it converts, so a host can apply daylight-saving rules.

`write` receives UTF-8. A lone surrogate in a JavaScript string arrives encoded as is (WTF-8), the bytes QuickJS writes for it.

`performance.now()` reads the process's monotonic clock, not the host.

## Hosts that ship with the engine

Three hosts are available.

- `quickjs_oxide::engine::api::core::SystemHost` is the trait's defaults: the process's streams and file system, the system clock, UTC. `Runtime::new()` and `Runtime::with_options` use it.
- `quickjs_oxide_host::SystemHost`, from the `quickjs-oxide-host` package in [`adapters/native`](https://github.com/pocket-nexus/quickjs-oxide/tree/main/adapters/native), adds the system's time zone. It follows the `TZ` environment variable (an IANA name or a POSIX rule) and otherwise the system's local zone, and seeds `Math.random` from the current time in microseconds, as QuickJS does. The `qjs` command uses it.
- `quickjs_oxide_web_host::WebHost`, from the `quickjs-oxide-web-host` package in [`adapters/web`](https://github.com/pocket-nexus/quickjs-oxide/tree/main/adapters/web), is for `wasm32-unknown-unknown`. It reads the browser's `Date.now()` and time zone, seeds from `Math.random()`, discards `write` output and has no files.

The two `SystemHost` types share a name. Import the one you mean by its full path.

## Creating a runtime over a host

A runtime takes its host by value when it is created.

| Constructor | Host | Options |
| --- | --- | --- |
| `Runtime::new()` | core `SystemHost` | `RuntimeOptions::default()` |
| `Runtime::with_options(options)` | core `SystemHost` | `options` |
| `Runtime::with_host(host, options)` | `host` | `options` |
| `Runtime::new_with_host_services(host)` | `host` | `RuntimeOptions::default()` |

The host must be `'static`. To read state back from it after the runtime has taken it, keep the state behind an `Rc<RefCell<…>>` and hold a second reference. [Errors and limits](./limits.md) covers `RuntimeOptions`.

<!-- include: examples/host_services.rs#system-host -->

## Capturing output

A host that overrides `write` receives everything `print` and `console.log` produce. This one keeps standard output in a shared buffer:

<!-- include: examples/host_services.rs#capture-output -->

`print` joins its arguments with spaces and ends the line with `\n`; each call is one `write`. A string argument is written as is, and any other value as `Runtime::print_value` formats it.

## Reproducible runs

A host with a fixed clock, a fixed time-zone offset and a fixed seed makes `Date` and `Math.random` return the same values on every run. That suits tests, snapshots and replays.

<!-- include: examples/host_services.rs#fixed-clock -->

## The qjs helpers

A new runtime's global object has the ECMAScript built-ins and nothing that does I/O. `Context::install_qjs_helpers` adds the globals that QuickJS's `qjs` defines, all of which go through the host:

- `print(...args)` and `console.log(...args)`, writing to `Stream::Stdout`;
- `performance.now()`;
- `scriptArgs`, an array of strings, when `QjsHelpers::script_args` is `Some`;
- `load(path)`, which runs a file as a script, and `read(path)`, which returns a file's text, when `QjsHelpers::std` is `true`.

`QjsHelpers::default()` installs the first group without `scriptArgs`, `load` or `read`.

`load` and `read` get their text from `Host::read_file`. A host that serves files from memory, an archive or a sandboxed directory overrides it. When `read_file` returns `None`, `load` throws a `ReferenceError` and `read` throws an `Error`.

<!-- include: examples/host_services.rs#read-file -->

## Promise rejection events

The engine calls `promise_rejection` for each operation of ECMAScript's `HostPromiseRejectionTracker`:

- `RejectionEvent::Rejected` when a promise is rejected with no handler attached;
- `RejectionEvent::Handled` when a handler is attached to a promise that was rejected without one.

The promise arrives as a `CoreValue`, the core's raw value. It holds no handle, so it does not keep the promise alive after the call returns. To inspect unhandled rejections from Rust, turn on rejection tracking and call `Runtime::take_unhandled_rejections` after the job queue drains; [Promises and jobs](./jobs.md#unhandled-rejections) shows how.

<!-- include: examples/host_services.rs#rejection-events -->

The host is called for every event whether tracking is on or off.

## Writing your own host

- Override only the methods you need; the rest keep the process defaults. To keep a script away from the real file system, override `read_file`, or do not install the helpers with `std: true`.
- A host method has no access to the runtime. It cannot run JavaScript or create values.
- A runtime and its host belong to one thread. `Runtime` holds `Rc` state and is not `Send`, so create both on the thread that uses them.

The complete program is [`examples/host_services.rs`](https://github.com/pocket-nexus/quickjs-oxide/blob/main/examples/host_services.rs):

```sh
cargo run --quiet --example host
```

## Where to go next

- [Promises and jobs](./jobs.md): drain the job queue and report unhandled rejections.
- [Modules](./modules.md): load modules through a `ModuleLoader`.
- [Errors and limits](./limits.md): runtime options, exceptions and the memory cap.
- [WebAssembly](./webassembly.md): run the engine in a browser with `WebHost`.
