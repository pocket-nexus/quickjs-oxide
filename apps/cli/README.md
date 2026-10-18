# qjs command-line application

Owns arguments, files, module-loading policy, diagnostics and process exits. It uses quickjs-oxide with the native adapter. CLI/oracle integration tests and fixed fixtures live under tests.

```sh
cargo test -p quickjs-oxide-cli --test cli
```

See [workspace architecture](../../docs/architecture.md) for dependency and lifetime boundaries.

## Diagnostics

`--gc-stats`, `--memory-report` and `--disassemble` work in every build;
`--ic-stats` needs the `ic-stats` feature, `--gc-stress=N` the `gc-stress`
feature, and `--test262` the `test262-host` feature. `qjs --help` lists the
options; [profiling documentation](../../docs/profiling.md) describes the
reports.
