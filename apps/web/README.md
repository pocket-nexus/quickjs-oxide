# Website and WebAssembly bindings

`src/` owns the WebAssembly exports: `evaluate` (one script in a fresh runtime), `Session` (one runtime kept across evaluations) and `engine_metadata`. Browser capabilities come from `adapters/web`.

`site/` is the homepage. Its demo runs `site/mandelbrot.js` in a `Session` inside a Web Worker (`site/worker.js`) and paints the results (`site/paint.js`); the Source view shows those same files.

`docs/` holds the docs pages: one Markdown file per entry of `docs/nav.mjs`. A line `<!-- include: examples/NAME.rs#REGION -->` embeds a region of a Cargo example, and `scripts/web/test-doc-examples.sh` runs every example a page includes.

```sh
npm ci
./scripts/web/build-site.sh   # WebAssembly build, homepage and docs into target/pages
./scripts/web/test-site.sh    # the build, a Node check of the bindings, the docs examples
npm run test:browser          # the homepage demo and every docs page in Chromium
```

See [workspace architecture](../../docs/architecture.md) for dependency and lifetime boundaries.
