// worker.js: the browser host. It loads quickjs-oxide, runs mandelbrot.js
// inside it, and hands each finished band to the page, which paints it.
/* global wasm_bindgen */
"use strict";

importScripts("./pkg/quickjs_oxide_web.js");

const STEPS = [8, 4, 2, 1];
const BAND_POINTS = 8000;   // about the same work per message in every pass
let session = null;
let currentJob = 0;

async function boot() {
  const start = performance.now();
  await wasm_bindgen({ module_or_path: "./pkg/quickjs_oxide_web_bg.wasm" });
  session = new wasm_bindgen.Session();          // one runtime, kept alive
  const source = await (await fetch("./mandelbrot.js")).text();
  check(session.eval(source));
  const meta = wasm_bindgen.engine_metadata();
  postMessage({ type: "ready", loadMs: performance.now() - start,
                version: meta.crateVersion, commit: meta.buildCommit });
}

function check(result) {
  if (!result.ok) throw new Error(result.text);
  return result.text;
}

async function render(job) {
  const { id, width, height, cx, cy, span, maxIter } = job;
  check(session.eval(`setView(${width}, ${height}, ${cx}, ${cy}, ${span}, ${maxIter}, ${STEPS[0]})`));
  let engineMs = 0, done = 0;
  for (const step of STEPS) {
    const rows = step * Math.max(1, Math.floor(BAND_POINTS / Math.ceil(width / step)));
    for (let y0 = 0; y0 < height; y0 += rows) {
      if (id !== currentJob) return;            // a newer run replaced this one
      const start = performance.now();
      const shades = check(session.eval(`renderBand(${step}, ${y0}, ${rows})`));
      engineMs += performance.now() - start;   // time inside the engine only
      const iterations = Number(check(session.eval("iterations")));
      done += countNew(shades);
      postMessage({ type: "band", id, step, y0, rows, shades, engineMs, iterations,
                    progress: done / (width * height) });
      await new Promise((resolve) => setTimeout(resolve, 0));  // let messages in
    }
  }
  postMessage({ type: "done", id });
}

function countNew(shades) {
  let n = 0;
  for (let i = 0; i < shades.length; i++) if (shades.charCodeAt(i) !== 256) n++;
  return n;
}

const booted = boot().catch((error) => postMessage({ type: "error", message: String(error.message || error) }));

onmessage = async ({ data: job }) => {
  currentJob = job.id;
  await booted;
  if (!session) return;
  try {
    await render(job);
  } catch (error) {
    postMessage({ type: "error", id: job.id, message: String(error.message || error) });
  }
};
