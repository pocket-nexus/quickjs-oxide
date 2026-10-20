import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { createRequire } from "node:module";
import path from "node:path";
import { pathToFileURL } from "node:url";

// The core resumes a delegated generator by re-entering its loop, so a
// yield* chain uses the host's stack; the web binding bounds re-entries at 600
// (apps/web/src/lib.rs), which a 400-deep chain stays within.
const deepYieldStarDepth = 400;
assert.deepEqual(process.argv.slice(2), []);

const require = createRequire(import.meta.url);
const metricsModulePath = path.resolve(
  process.cwd(),
  "scripts/test262/current-test262-metrics.mjs",
);
const { parseCurrentTest262Metrics } = await import(
  pathToFileURL(metricsModulePath)
);
const currentSpec = await readFile(
  path.resolve(process.cwd(), "dev-support/test262/current.conf"),
  "utf8",
);
const currentMetrics = parseCurrentTest262Metrics(currentSpec);
const workspaceManifest = await readFile(
  path.resolve(process.cwd(), "Cargo.toml"),
  "utf8",
);
const workspaceVersion = workspaceManifest.match(
  /^\[workspace\.package\][^[]*?^version = "([^"]+)"$/msu,
)?.[1];
assert.ok(workspaceVersion, "Cargo.toml has no [workspace.package] version");
const renderedIndex = await readFile(
  path.resolve(process.cwd(), "target/pages/index.html"),
  "utf8",
);
// The homepage shows the QuickJS-scope figures of the frozen vector.
assert.ok(renderedIndex.includes(
  `${Number(currentMetrics.quickJsScopePercent).toFixed(2)}%`,
));
assert.ok(renderedIndex.includes(
  `${currentMetrics.fullPasses.toLocaleString("en-US")} of ${currentMetrics.quickJsScope.toLocaleString("en-US")}`,
));
assert.ok(!renderedIndex.includes("@@"), "unrendered placeholder in index.html");
assert.throws(
  () => parseCurrentTest262Metrics(`${currentSpec}\nfull_passes=1\n`),
  /duplicate Test262 spec key full_passes/u,
);
assert.throws(
  () => parseCurrentTest262Metrics(
    currentSpec.replace(
      `pass=${currentMetrics.fullPasses}`,
      `pass=${currentMetrics.fullPasses - 1}`,
    ),
  ),
  /summaries disagree with the official metrics/u,
);

const wrapperPath = path.resolve(
  process.cwd(),
  "target/web-site-node/quickjs_oxide_web.js",
);
const { engine_metadata: engineMetadata, evaluate, Session } = require(wrapperPath);

// The homepage demo: one session evaluates mandelbrot.js once and then
// renders every pass, exactly as apps/web/site/worker.js drives it.
const kernel = await readFile(path.resolve(process.cwd(), "apps/web/site/mandelbrot.js"), "utf8");
const session = new Session();
assert.equal(session.eval(kernel).ok, true);
const width = 64, height = 48, steps = [8, 4, 2, 1];
assert.equal(session.eval(`setView(${width}, ${height}, -0.75, 0, 3.3, 100, ${steps[0]})`).ok, true);
let computed = 0;
for (const step of steps) {
  const band = session.eval(`renderBand(${step}, 0, ${height})`);
  assert.equal(band.ok, true, band.text);
  assert.equal(band.kind, "string");
  assert.equal(band.text.length, Math.ceil(width / step) * Math.ceil(height / step));
  for (const char of band.text) {
    const shade = char.charCodeAt(0);
    assert.ok(shade <= 256, `shade ${shade} out of range`);
    if (shade !== 256) computed++;
  }
}
assert.equal(computed, width * height, "every pixel is computed exactly once");
const iterations = session.eval("iterations");
assert.equal(iterations.kind, "number");
assert.ok(Number(iterations.text) > width * height);
const sessionError = session.eval("renderBand(");
assert.equal(sessionError.ok, false);
assert.match(sessionError.text, /^SyntaxError:/);
assert.equal(session.eval("typeof renderBand").text, "function", "a failed eval keeps the session");
session.free();

const metadata = engineMetadata();
assert.deepEqual(
  {
    engine: metadata.engine,
    crateVersion: metadata.crateVersion,
    quickjsTarget: metadata.quickjsTarget,
    buildCommit: metadata.buildCommit,
    canBlock: metadata.canBlock,
  },
  {
    engine: "quickjs-oxide",
    crateVersion: workspaceVersion,
    quickjsTarget: "QuickJS 2026-06-04",
    buildCommit:
      process.env.QUICKJS_OXIDE_COMMIT || process.env.GITHUB_SHA || "local",
    canBlock: false,
  },
);

const qjsHostIsolation = evaluate(
  '(typeof print) + "|" + (typeof console)',
);
assert.deepEqual(
  {
    ok: qjsHostIsolation.ok,
    kind: qjsHostIsolation.kind,
    text: qjsHostIsolation.text,
  },
  { ok: true, kind: "string", text: "undefined|undefined" },
);

const evalVarDestructuring = evaluate(`
  (function () {
    eval("var { answer = function () { return 42; } } = {};");
    return answer.name === "answer" ? answer() : 0;
  })()
`);
assert.deepEqual(
  {
    ok: evalVarDestructuring.ok,
    kind: evalVarDestructuring.kind,
    text: evalVarDestructuring.text,
  },
  { ok: true, kind: "number", text: "42" },
);

const deepYieldStar = evaluate(`
  (function () {
    function* chain(depth) {
      return yield* (depth ? chain(depth - 1) : [42]);
    }
    return chain(${deepYieldStarDepth}).next().value;
  })()
`);
assert.deepEqual(
  { ok: deepYieldStar.ok, kind: deepYieldStar.kind, text: deepYieldStar.text },
  { ok: true, kind: "number", text: "42" },
);

const caughtYieldStarOverflow = evaluate(`
  (function () {
    function* chain(depth) {
      return yield* (depth ? chain(depth - 1) : [42]);
    }
    var observed;
    try {
      chain(Infinity).next();
      observed = "missing";
    } catch (error) {
      observed = error.name + ":" + error.message;
    }
    return observed + "|" + 6 * 7;
  })()
`);
assert.deepEqual(
  {
    ok: caughtYieldStarOverflow.ok,
    kind: caughtYieldStarOverflow.kind,
    text: caughtYieldStarOverflow.text,
  },
  { ok: true, kind: "string", text: "InternalError:stack overflow|42" },
);

const syntaxError = evaluate("function () {");
assert.equal(syntaxError.ok, false);
assert.equal(syntaxError.kind, "exception");
assert.match(syntaxError.text, /^SyntaxError:/);

console.log(
  `Node/WASM smoke: the homepage kernel rendered ${width}×${height} in one session (${iterations.text} iterations), Test262 figures and build metadata passed; deep yield-star overflow stayed catchable`,
);
