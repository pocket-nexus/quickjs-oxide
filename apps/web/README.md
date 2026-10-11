# Web playground application

Owns WASM exports, playground evaluation options and JS-compatible result conversion. Browser capabilities are supplied by adapters/web. UI assets live in `site/`; build and browser acceptance scripts live in `scripts/web`.

```sh
./scripts/web/test-web-playground.sh
```

See [workspace architecture](../../docs/architecture.md) for dependency and lifetime boundaries.
