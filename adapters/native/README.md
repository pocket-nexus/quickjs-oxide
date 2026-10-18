# Native host adapter

Implements the engine's `Host` trait as `SystemHost`, using native time, timezone rules, random seeding and stdout. Package name is `quickjs-oxide-host`; JavaScript semantics live in the engine. Native application entry points pass `SystemHost` to `Runtime::new_with_host_services` or `Runtime::with_host`.

```sh
cargo test -p quickjs-oxide-host
```

See [workspace architecture](../../docs/architecture.md) for dependency and lifetime boundaries.

`engine-test-support` lets engine unit tests identify the `Host` trait in Cargo's separate engine instance; production callers use `Host` from `quickjs_oxide::engine::api::core`.
