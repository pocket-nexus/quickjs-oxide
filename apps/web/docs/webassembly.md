# WebAssembly

quickjs-oxide compiles to `wasm32-unknown-unknown`. The `quickjs-oxide-web` crate (`apps/web`) wraps the engine in a small wasm-bindgen API, and the browser runs it like any other WebAssembly module. JavaScript you hand it runs inside quickjs-oxide, not in the browser's own `eval` or `Function`.

This page covers building the package, the JavaScript API it exports, the browser host, and a worked example: the Mandelbrot demo on the homepage.

## Get the package

Each [GitHub release](https://github.com/pocket-nexus/quickjs-oxide/releases) carries `quickjs-oxide-wasm-vVERSION.tar.gz`, with two wasm-bindgen builds of the same module:

| Directory | wasm-bindgen target | Use it from |
| --- | --- | --- |
| `web/` | `web` | ES modules in the browser |
| `nodejs/` | `nodejs` | Node.js `require` |

The homepage demo uses a third target, `no-modules`, which suits a classic Web Worker loaded with `importScripts`. To get that build, or any build of your own checkout, build from source.

## Build from source

The build uses the toolchain pinned in `rust-toolchain.toml` (Rust 1.99.0) and the wasm-bindgen CLI at exactly the version the crate pins, 0.2.126:

```sh
rustup target add --toolchain 1.99.0 wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.126 --locked
```

Build the crate with the `web` profile, then generate the JavaScript glue:

```sh
cargo build --locked --profile web --target wasm32-unknown-unknown -p quickjs-oxide-web
wasm-bindgen target/wasm32-unknown-unknown/web/quickjs_oxide_web.wasm \
  --out-dir pkg --out-name quickjs_oxide_web --target no-modules --no-typescript
```

This writes `pkg/quickjs_oxide_web.js` (the glue) and `pkg/quickjs_oxide_web_bg.wasm` (the engine). Pass `--target web` or `--target nodejs` instead for the module or Node.js forms.

The `web` profile is defined in the workspace `Cargo.toml`. It inherits `release` and optimizes for size:

```toml
[profile.web]
inherits = "release"
opt-level = "s"
lto = true
codegen-units = 1
strip = true
panic = "abort"
```

[`scripts/web/build-site.sh`](https://github.com/pocket-nexus/quickjs-oxide/blob/main/scripts/web/build-site.sh) runs the same two steps for the published site, then builds the homepage and these docs around the result. It first checks that the pinned toolchain, the `wasm32-unknown-unknown` target and wasm-bindgen 0.2.126 are installed, and prints the install command for whatever is missing. The build reads `QUICKJS_OXIDE_COMMIT` and reports it as the build commit; without it the commit is `local`.

## The JavaScript API

The module exports two functions and one class. With the `no-modules` target they hang off the global `wasm_bindgen`; with `web` and `nodejs` they are named exports.

| Export | Returns | What it does |
| --- | --- | --- |
| `evaluate(source)` | result | Runs one script in a fresh runtime that is dropped afterwards |
| `engine_metadata()` | metadata | Describes the loaded build |
| `new Session()` | `Session` | Creates one runtime that stays alive |
| `session.eval(source)` | result | Runs a script in that session's runtime |

### Results

`evaluate` and `session.eval` return a plain object with three fields:

```text
{ ok: boolean, kind: string, text: string }
```

| `kind` | `ok` | `text` |
| --- | --- | --- |
| `undefined`, `null`, `boolean`, `number`, `bigint`, `string`, `symbol` | `true` | The completion value as a string (`42`, `10n`, `Symbol(tag)`) |
| `object` | `true` | `[object Object]` |
| `exception` | `false` | The thrown error as `Name: message`, or the thrown value |
| `engine-error` | `false` | The message of an engine error that is not a JavaScript exception |

The completion value of a script is the value of its last expression statement. An object comes back as `[object Object]`, so return structured data as a string, for example with `JSON.stringify`.

Before returning, both calls run every pending job, so the promise reactions the script queued have run.

### Sessions

A `Session` holds one runtime and its one realm. Globals, functions and objects defined by one `eval` are there for the next, so a host can load a script once and call into it many times:

```js
importScripts("./pkg/quickjs_oxide_web.js");

async function main() {
  await wasm_bindgen({ module_or_path: "./pkg/quickjs_oxide_web_bg.wasm" });
  const session = new wasm_bindgen.Session();
  session.eval("function add(a, b) { return a + b; }");
  // { ok: true, kind: "undefined", text: "undefined" }
  session.eval("add(40, 2)");
  // { ok: true, kind: "number", text: "42" }
  session.eval("nope");
  // { ok: false, kind: "exception", text: "ReferenceError: 'nope' is not defined" }
}
main();
```

`evaluate(source)` builds a fresh runtime for every call, so nothing carries over between calls.

### Metadata

`engine_metadata()` returns the identity of the loaded build:

```text
{ engine: "quickjs-oxide", crateVersion: "0.3.1", quickjsTarget: "QuickJS 2026-06-04",
  buildCommit: "local", canBlock: false }
```

`buildCommit` is the value of `QUICKJS_OXIDE_COMMIT` at build time. `canBlock` is the runtime's blocking policy: it is `false`, so `Atomics.wait` throws a `TypeError` instead of blocking the worker.

## The browser host

The engine reaches its environment through the `Host` trait ([Host services](./host.md)). The `quickjs-oxide-web-host` crate (`adapters/web`) implements it as `WebHost`:

| Host service | `WebHost` |
| --- | --- |
| Clock (`Date.now`, `new Date()`) | The browser's `Date.now()` |
| Local time zone offset | The browser's `Date.prototype.getTimezoneOffset()` |
| `Math.random` seed | Two calls to the browser's `Math.random()` |
| Output (`print`, `console.log`) | Discarded |
| File reads | None |

Output is discarded, so a script reports back through its completion value. The demo below returns a string from each call.

`apps/web` creates every runtime with `Runtime::with_host(WebHost, options)` and turns blocking off with `set_can_block(false)`. The engine measures how deep native code re-enters JavaScript from stack addresses, and in WebAssembly those addresses belong to linear memory, not to the browser's machine stack. The web runtime therefore sets two limits in `RuntimeOptions`:

| Option | Value | Purpose |
| --- | --- | --- |
| `native_depth_limit` | 600 | Nested re-entries from native code into JavaScript |
| `native_stack_budget` | 512 KiB | Half of the 1 MiB linear-memory stack that `wasm-ld` reserves |

Deep recursion through callbacks, getters, proxy traps or generators ends in a catchable `InternalError: stack overflow`. [Errors and limits](./limits.md) covers the other limits.

## Run it in a Web Worker

A script can run for a long time, and while it runs the thread that called `eval` is blocked. Run the engine in a dedicated Web Worker so the page keeps drawing and handling input, and talk to it with `postMessage`.

The homepage demo splits the work three ways:

| File | Runs in | Role |
| --- | --- | --- |
| `mandelbrot.js` | quickjs-oxide | Computes the image, one band of rows per call |
| `worker.js` | The Web Worker | Loads quickjs-oxide, calls into `mandelbrot.js`, posts each band |
| `paint.js` | The page | Paints each band onto a canvas |

### The script quickjs-oxide runs

`mandelbrot.js` defines two functions and some globals. The host calls `setView` once per image, then `renderBand` for each band. It renders in passes: every 8th pixel, then every 4th, 2nd and finally all of them. Each call returns its band as a string with one character per point, so the result crosses the WebAssembly boundary as the `text` of a string completion. A global, `iterations`, counts the work done.

<!-- include: apps/web/site/mandelbrot.js -->

### The host

`worker.js` loads the glue with `importScripts`, initializes the module, creates one `Session` and evaluates `mandelbrot.js` in it. For each job it builds small call expressions such as `renderBand(8, 0, 24)` and evaluates them in the same session, so the functions are compiled once. `check` turns a failed result into a JavaScript exception. Between bands it yields to the event loop with `setTimeout`, so a newer job posted by the page can replace the current one.

<!-- include: apps/web/site/worker.js -->

### The page

`paint.js` runs on the page. It maps each shade character to a color and fills a block of pixels per point, so a coarse pass shows up at once and finer passes sharpen it. It knows nothing about quickjs-oxide; it reads the bands the worker posts.

<!-- include: apps/web/site/paint.js -->

The page starts the worker with `new Worker("worker.js")`, posts a job object (`id`, `width`, `height`, `cx`, `cy`, `span`, `maxIter`) and calls `paint` for each `band` message on an `ImageData` frame it then draws to the canvas.

## Where to go next

- [Host services](./host.md): the `Host` trait that `WebHost` implements.
- [Errors and limits](./limits.md): exceptions, engine errors and the runtime's limits.
- [Embed quickjs-oxide](./embed.md): the Rust API that `apps/web` wraps.
- [`apps/web/src/lib.rs`](https://github.com/pocket-nexus/quickjs-oxide/blob/main/apps/web/src/lib.rs): the complete bindings.
