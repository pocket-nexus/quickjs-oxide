# Embed quickjs-oxide

quickjs-oxide is a JavaScript engine for Rust applications. It is written in Rust with no unsafe code: every crate in the workspace sets `unsafe_code = "forbid"`. ECMA-262 and Test262 decide its behavior; QuickJS 2026-06-04 decides what the specification leaves to the implementation, such as error messages and the printer.

An application creates a `Runtime`, evaluates JavaScript through its `Context`, and reads the results as Rust values. The engine reaches the outside world only through a `Host` (output, clock, time zone, random seed, file reads, promise-rejection events), so the application decides what scripts can see.

| What you want to do | Where to start |
| --- | --- |
| Run a first script from Rust | this page, sections 1 to 4 |
| Read, build and pass JavaScript values | [Values and calls](./values.md) |
| Control output, time, randomness and files | [Host services](./host.md) |
| Load ES modules from your own sources | [Modules](./modules.md) |
| Drive promises and `async` code | [Promises and jobs](./jobs.md) |
| Handle exceptions, cap memory and stack | [Errors and limits](./limits.md) |
| Run the engine in a browser | [WebAssembly](./webassembly.md) |
| Run scripts and modules from the command line | [Use the qjs command line](./cli.md) |
| Check which JavaScript features are supported | [JavaScript support](./compatibility.md) |

## The crates

| Crate | Use it for |
| --- | --- |
| `quickjs-oxide` | The engine and its embedding API, under `quickjs_oxide::engine::api::core`. |
| `quickjs-oxide-host` | `SystemHost`: the system clock, the host's time-zone rules (`TZ` or `/etc/localtime`), stdout and stderr, files. |
| `quickjs-oxide-web-host` | `WebHost`: the browser's clock, time zone and random source, for `wasm32` builds. |
| `quickjs-oxide-cli` | The `qjs` command line, built on the two crates above. |

## 1. Add the crates

The crates are on [crates.io](https://crates.io/crates/quickjs-oxide), with their API documentation on [docs.rs](https://docs.rs/quickjs-oxide). They need Rust 1.99 or newer.

```toml
[dependencies]
quickjs-oxide = "0.4"
quickjs-oxide-host = "0.4"
```

Everything an embedder uses lives in one module:

<!-- include: examples/embed.rs#imports -->

## 2. Evaluate a script

A `Runtime` owns one heap, one realm and one job queue. `new_context` returns the handle of its realm, and `eval` compiles and runs a script and returns its completion value.

<!-- include: examples/embed.rs#first-eval -->

`Runtime::new_with_host_services` takes the host. `SystemHost` from `quickjs-oxide-host` reads the system clock and the local time zone; `Runtime::new()` uses a built-in host that reports UTC.

**A runtime is single-threaded:** `Runtime` and `Context` are reference-counted handles that stay on the thread that created them. Separate runtimes share nothing, so an application that needs isolation creates one runtime per tenant.

A fresh realm has the ECMAScript built-ins and nothing else. The `qjs` helpers add `print`, `console.log` and `scriptArgs`; their output goes through the host's `write`.

<!-- include: examples/embed.rs#console -->

## 3. Read the result

`eval` returns a `Value`. Primitives arrive as Rust data: numbers as `Int(i32)` or `Float(f64)`, strings as `JsString`, BigInts as `JsBigInt`. Objects and symbols arrive as handles (`ObjectRef`, `SymbolRef`) that keep the JavaScript value alive while Rust holds them.

<!-- include: examples/embed.rs#read-result -->

A property name is a `PropertyKey`. `Runtime::intern_property_key` makes one from a Rust string, and `PropertyKey::index` makes an array index. [Values and calls](./values.md) covers every variant and conversion.

When the script throws, `eval` returns `Err(RuntimeError::Exception(value))` with the thrown value.

<!-- include: examples/embed.rs#exception -->

## 4. Call a JavaScript function from Rust

A function is an object; `Runtime::as_callable` turns an `ObjectRef` into a `CallableRef` when the object is callable. `Context::call` takes the function, the `this` value and the arguments.

<!-- include: examples/embed.rs#call-function -->

JavaScript reaches the application only through the `Host` trait, described in [Host services](./host.md).

## 5. Run the same script with qjs

The `qjs` command line runs files and expressions with the same engine, so a script can be tried before it is embedded. Install it from crates.io, or download a prebuilt binary from the [GitHub releases](https://github.com/pocket-nexus/quickjs-oxide/releases):

```sh
cargo install --locked quickjs-oxide-cli
```

Save a script as `double.js`:

```js
const doubled = [1, 2, 3].map((x) => x * 2).join();
console.log(doubled);
```

```sh
qjs double.js                      # 2,4,6
qjs --print-result -e '6 * 7'      # 42
qjs -e 'null.length'               # prints the TypeError, exits with status 1
qjs module.mjs                     # .mjs files run as ES modules
```

`qjs` installs the same helpers as `install_qjs_helpers`, a file module loader and promise-rejection tracking, then drains the job queue before it exits. [Use the qjs command line](./cli.md) covers its options and how it follows QuickJS's `qjs`.

## Where to go next

- [Values and calls](./values.md): every `Value` variant, strings, BigInts, symbols, objects, arrays, calls and constructors.
- [Host services](./host.md): implement `Host` to route output, time, randomness and file reads.
- [Modules](./modules.md): install a `ModuleLoader` and run ES modules.
- [Promises and jobs](./jobs.md): run the job queue and read promise results.
- [Errors and limits](./limits.md): `RuntimeError`, heap caps and stack limits.
- [Use the qjs command line](./cli.md): run scripts and modules with `qjs`, which follows QuickJS's `qjs`.
- [`examples/embed.rs`](https://github.com/pocket-nexus/quickjs-oxide/blob/main/examples/embed.rs): this page's code as one runnable file (`cargo run --example embed`).
