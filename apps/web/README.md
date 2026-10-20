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

## Deployment

`.github/workflows/site.yml` deploys the site on every push to `main`:

1. `build` builds the site into `target/pages` and runs the site tests (the WebAssembly build, the Node check, the docs examples and the browser checks).
2. `deploy` runs `wrangler deploy --config apps/web/wrangler.jsonc`: the Worker `quickjs-oxide` serves `target/pages` as static assets, with no script, at its custom domain <https://quickjs-oxide.pocket.nexus/>. It authenticates with the repository secret `CLOUDFLARE_API_TOKEN`, a token with the Workers Scripts edit permission on the account `wrangler.jsonc` names.
3. `verify` runs `scripts/web/test-live-pages.mjs` against the domain: the homepage, the WebAssembly glue and module byte for byte, and the docs of this commit.
4. `redirect` deploys `scripts/web/build-redirect.mjs`'s page to GitHub Pages, the site's former address, `pocket-nexus.github.io/quickjs-oxide/`: every path there goes to the same path on the domain.

The Worker reads the build's `_redirects` file, which sends `/docs/` to the first docs page. Assets carry a content digest in their URL (`?v=`), so a deployment replaces them without stale caches. Static asset requests run no script and are not billed as Worker requests.

The same deployment from a local checkout, after `./scripts/web/build-site.sh`, is `npx wrangler deploy --config apps/web/wrangler.jsonc`.

See [workspace architecture](../../docs/architecture.md) for dependency and lifetime boundaries.
