# Use the qjs command line

`qjs` is quickjs-oxide's command line. **It follows the `qjs` of QuickJS 2026-06-04**: the same usage line, the same options for running scripts and modules, the same `print`, `console.log` and `scriptArgs`, the same error output and the same exit statuses. If you have used QuickJS's `qjs`, you already know this one; the [differences](#differences-from-quickjss-qjs) are listed at the end of this page.

## Install

Prebuilt binaries for Linux, macOS and Windows are attached to every [GitHub release](https://github.com/pocket-nexus/quickjs-oxide/releases). With Rust installed, build it from crates.io or fetch the prebuilt binary through cargo-binstall:

```sh
cargo install --locked quickjs-oxide-cli
cargo binstall quickjs-oxide-cli
```

```sh
qjs --version
# quickjs-oxide 0.4.0 (QuickJS 2026-06-04 compatibility target)
```

## Run a script

```text
usage: qjs [options] [file [args]]
```

`qjs` runs `file` and passes the arguments after it to the script as `scriptArgs`, with the file name first:

```js
// main.js
import { x } from "./lib.mjs";
console.log(x * 2, scriptArgs.join("|"));
```

```sh
qjs main.js a b        # 42 main.js|a|b
```

As in QuickJS, a file is loaded as an ES module when its name ends in `.mjs` or its source uses `import` or `export`, and as a script otherwise. `-m` / `--module` and `--script` force one or the other. Modules are loaded from the file system relative to the importing file.

## Evaluate an expression

`-e` evaluates its argument instead of a file. `--print-result` prints the completion value of the script:

```sh
qjs -e 'print(6 * 7)'              # 42
qjs --print-result -e '6 * 7'      # 42
```

## Errors and exit status

An uncaught exception prints the error and its stack, the way QuickJS prints it, and exits with status 1:

```sh
qjs -e 'null.length'
# TypeError: cannot read property 'length' of null
#     at <eval> (<cmdline>:1:5)
```

A promise that is rejected and never handled prints `Possibly unhandled promise rejection:` and exits with status 1; `--no-unhandled-rejection` ignores such rejections. Status 0 means the script and every job it queued completed. An unknown option prints the usage text and exits with status 2.

## Options

| Option | Effect |
| --- | --- |
| `-e`, `--eval EXPR` | evaluate `EXPR` |
| `-m`, `--module` | load as an ES module |
| `--script` | load as a script |
| `-s` | strip all debug information |
| `--strip-source` | strip only function source text |
| `--no-unhandled-rejection` | ignore unhandled promise rejections |
| `--print-result` | print the script's completion value |
| `-q`, `--quit` | create the runtime and exit |
| `--std` | define the `load(file)` and `read(file)` globals |
| `-v`, `--version` | show the version and the QuickJS version it targets |
| `-h`, `--help` | show the usage text |

### Memory and measurement

| Option | Effect |
| --- | --- |
| `--heap-cap-mib N` | cap the heap at `N` MiB; past it, allocation throws `InternalError: out of memory` |
| `--min-heap-mib N` | the collector's minimum heap budget (default 1) |
| `--gc-growth F` | bytes allocated between collections per live byte (default 2) |
| `--stack-limit N` | the number of registers the register file may hold |
| `--gc-stats` | print the collector's statistics at exit |
| `--memory-report` | print what every part of the runtime holds at exit |
| `--disassemble` | print the register code of `file` and exit |

[Errors and limits](./limits.md) explains the heap and collector settings, and [Optimizations](./optimizations.md) the register code that `--disassemble` prints.

## Differences from QuickJS's qjs

| QuickJS's `qjs` | quickjs-oxide's `qjs` |
| --- | --- |
| `--std` makes the `std` and `os` modules available | `--std` defines the `load` and `read` globals |
| `--memory-limit n` | `--heap-cap-mib N` |
| `-d` (memory usage at exit) | `--memory-report` |
| `-i` (interactive mode), `-I` (include a file), `--strict`, `-T` (trace allocations), `--stack-size n` | not available |
| an unknown option exits with status 1 | an unknown option exits with status 2 |

## Where to go next

- [Embed quickjs-oxide](./embed.md): run the same engine from a Rust application.
- [JavaScript support](./compatibility.md): Test262 results and the language differences from QuickJS.
- [Performance](./performance.md): how `qjs` compares with QuickJS's on the benchmark suites.
