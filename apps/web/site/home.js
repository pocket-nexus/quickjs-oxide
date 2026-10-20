"use strict";
const HL_KEYWORDS = /\b(const|let|var|function|return|for|while|if|else|import|from|export|async|await|new|of|fn|use|mut|match|Ok|Some|None|self|true|false)\b/;
function highlight(src) {
  const re = /(\/\/[^\n]*)|("(?:[^"\\]|\\.)*"|'(?:[^'\\]|\\.)*'|`[^`]*`)|\b(\d+(?:\.\d+)?)\b|\b([A-Za-z_][\w]*)(?=\s*\()|\b([A-Za-z_]\w*)\b/g;
  let out = "", last = 0, m;
  const esc = (s) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
  while ((m = re.exec(src))) {
    out += esc(src.slice(last, m.index));
    if (m[1]) out += '<span class="c">' + esc(m[1]) + "</span>";
    else if (m[2]) out += '<span class="s">' + esc(m[2]) + "</span>";
    else if (m[3]) out += '<span class="n">' + m[3] + "</span>";
    else if (m[4]) out += HL_KEYWORDS.test(m[4]) && m[4].length === m[4].match(HL_KEYWORDS)[0].length ? '<span class="k">' + m[4] + "</span>" : '<span class="f">' + m[4] + "</span>";
    else out += HL_KEYWORDS.test(m[5]) && m[5] === m[5].match(HL_KEYWORDS)[0] ? '<span class="k">' + m[5] + "</span>" : m[5];
    last = re.lastIndex;
  }
  return out + esc(src.slice(last));
}
document.querySelectorAll("pre[data-hl]").forEach((pre) => { pre.innerHTML = highlight(pre.textContent); });

const SOURCE_FILES = { js: "mandelbrot.js", host: "worker.js", paint: "paint.js" };
const SOURCES = {};
const panel = document.querySelector("[data-panel]");
const canvas = panel.querySelector("canvas");
const source = panel.querySelector(".source");
const code = source.querySelector(".code");
const picker = source.querySelector("select");
// The Source view shows the very files this page runs.
async function showSource(which) {
  if (!(which in SOURCES)) SOURCES[which] = await (await fetch(SOURCE_FILES[which])).text();
  code.innerHTML = highlight(SOURCES[which]);
  code.scrollTop = 0;
}
picker.addEventListener("change", () => showSource(picker.value));
showSource("js");
panel.querySelectorAll(".tab").forEach((tab) => tab.addEventListener("click", () => {
  panel.querySelectorAll(".tab").forEach((t) => t.setAttribute("aria-selected", String(t === tab)));
  const demo = tab.dataset.view === "demo";
  canvas.hidden = !demo; source.hidden = demo;
  panel.querySelectorAll("[data-demo-only]").forEach((el) => { el.style.visibility = demo ? "" : "hidden"; });
}));
source.querySelector("[data-copy]").addEventListener("click", async (e) => {
  try { await navigator.clipboard.writeText(SOURCES[picker.value]); e.target.textContent = "Copied"; }
  catch { const r = document.createRange(); r.selectNodeContents(code); getSelection().removeAllRanges(); getSelection().addRange(r); }
  setTimeout(() => { e.target.textContent = "Copy"; }, 1400);
});
panel.querySelectorAll(".seg button").forEach((b) => b.addEventListener("click", () => {
  panel.querySelectorAll(".seg button").forEach((x) => x.setAttribute("aria-pressed", String(x === b)));
}));


const ctx = canvas.getContext("2d");
const stats = { progress: panel.querySelector("[data-progress]"), time: panel.querySelector("[data-time]"),
  iters: panel.querySelector("[data-iters]"), state: panel.querySelector(".state"), bar: panel.querySelector("[data-bar]") };
const VIEW = {"cx": -0.75, "cy": 0, "span": 3.3}, LEVELS = {"Low": {"width": 480, "height": 360, "maxIter": 200}, "Mid": {"width": 640, "height": 480, "maxIter": 300}, "High": {"width": 960, "height": 720, "maxIter": 500}};
const meta = (k) => panel.querySelector('[data-meta="' + k + '"]');
let level = "Mid", jobId = 0, job = null, frame = null;
function setState(text, done) { stats.state.classList.toggle("done", !!done); stats.state.lastChild.textContent = text; }
function show(progress, ms, iters) {
  stats.progress.textContent = Math.round(progress * 100) + "%";
  stats.time.textContent = (ms / 1000).toFixed(2) + " s";
  stats.iters.textContent = iters.toLocaleString("en-US");
  if (stats.bar) stats.bar.style.setProperty("--p", progress);
}
const worker = new Worker("worker.js");
setState("Loading engine");
function run() {
  job = Object.assign({ id: ++jobId }, VIEW, LEVELS[level]);
  canvas.width = job.width; canvas.height = job.height;
  ctx.fillStyle = "#120c21"; ctx.fillRect(0, 0, job.width, job.height);
  frame = ctx.getImageData(0, 0, job.width, job.height);
  show(0, 0, 0);
  meta("size").textContent = job.width + " × " + job.height + ", " + job.maxIter + " max iterations";
  if (worker.ready) setState("Computing");
  worker.postMessage(job);
}
worker.onmessage = ({ data }) => {
  if (data.type === "ready") {
    worker.ready = true;
    meta("engine").textContent = "quickjs-oxide " + data.version + " · wasm32";
    meta("commit").textContent = data.commit;
    meta("load").textContent = Math.round(data.loadMs) + " ms";
    setState("Computing");
    return;
  }
  if (data.type === "error") { setState("Error"); console.error(data.message); return; }
  if (data.id !== jobId) return;
  if (data.type === "band") {
    paint(frame, data);
    ctx.putImageData(frame, 0, 0, 0, data.y0, job.width, data.rows);
    show(data.progress, data.engineMs, data.iterations);
  } else if (data.type === "done") setState("Done", true);
};
panel.querySelectorAll(".seg button").forEach((b) => b.addEventListener("click", () => {
  level = b.textContent;
  run();
}));
panel.querySelector("[data-run]").addEventListener("click", run);
run();
