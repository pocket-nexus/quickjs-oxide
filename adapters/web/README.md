# Browser host adapter

Implements the engine's `Host` trait as `WebHost`, using browser Date and Math services. WASM product exports and result formatting belong to apps/web. Package name is `quickjs-oxide-web-host`.

```sh
cargo check -p quickjs-oxide-web-host --target wasm32-unknown-unknown
```

See [workspace architecture](../../docs/architecture.md) for dependency and lifetime boundaries.
