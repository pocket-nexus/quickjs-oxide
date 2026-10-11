# Native host adapter

Implements the engine HostServices contract using native time, timezone rules, random seeding and stdout. Package name is `quickjs-oxide-host`; JavaScript semantics live in the engine. Native application entry points pass SystemHostServices to Runtime::new_with_host_services.

```sh
cargo test -p quickjs-oxide-host
```

See [workspace architecture](../../docs/architecture.md) for dependency and lifetime boundaries.

`engine-test-support` 仅供引擎单元测试识别 Cargo 的独立 trait 实例；生产调用方从 `quickjs_oxide::engine::api` 使用 HostServices。
