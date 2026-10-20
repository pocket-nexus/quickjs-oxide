// Loads the built site (target/pages) in Chromium: the homepage's demo runs
// quickjs-oxide to completion at every detail level, the Source view shows the
// files the demo runs, and every docs page renders without errors, without
// sideways scrolling, and with working internal links.
//
//   node scripts/web/test-site-browser.mjs
//
// CHROMIUM_PATH selects a system browser instead of Playwright's own.

import assert from "node:assert/strict";
import { readFile, stat } from "node:fs/promises";
import { createServer } from "node:http";
import path from "node:path";
import process from "node:process";
import { chromium } from "playwright";
import { DOC_NAV } from "../../apps/web/docs/nav.mjs";

const ROOT = path.resolve(import.meta.dirname, "../..");
const PAGES = path.join(ROOT, "target/pages");
const TYPES = {
  ".html": "text/html; charset=utf-8", ".js": "text/javascript", ".css": "text/css",
  ".wasm": "application/wasm", ".svg": "image/svg+xml", ".png": "image/png",
};

assert.deepEqual(process.argv.slice(2), [], "usage: test-site-browser.mjs");
await stat(path.join(PAGES, "index.html")).catch(() => {
  throw new Error("target/pages is missing; run scripts/web/build-site.sh first");
});

const server = createServer(async (request, response) => {
  const url = new URL(request.url, "http://localhost");
  let file = path.join(PAGES, decodeURIComponent(url.pathname));
  if (!file.startsWith(PAGES)) return response.writeHead(403).end();
  if (url.pathname.endsWith("/")) file = path.join(file, "index.html");
  try {
    const body = await readFile(file);
    response.writeHead(200, { "content-type": TYPES[path.extname(file)] ?? "application/octet-stream" }).end(body);
  } catch {
    response.writeHead(404).end();
  }
});
await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
const base = `http://127.0.0.1:${server.address().port}/`;

const browser = await chromium.launch({ headless: true, executablePath: process.env.CHROMIUM_PATH || undefined });
let failures = 0;
async function check(name, body) {
  try {
    await body();
    console.log(`ok - ${name}`);
  } catch (error) {
    failures++;
    console.log(`not ok - ${name}\n  ${String(error.stack ?? error).replace(/\n/g, "\n  ")}`);
  }
}

async function open(url, viewport) {
  const page = await browser.newPage({ viewport });
  const errors = [];
  page.on("console", (message) => { if (message.type() === "error") errors.push(message.text()); });
  page.on("pageerror", (error) => errors.push(String(error)));
  page.on("response", (response) => {
    if (response.status() >= 400) errors.push(`${response.status()} ${response.url()}`);
  });
  await page.goto(new URL(url, base).href);
  return { page, errors };
}

const stateText = (page) => page.locator("[data-panel] .state").innerText();
async function waitDone(page) {
  await page.waitForFunction(() => /done|error/i.test(document.querySelector("[data-panel] .state").textContent), null, { timeout: 120_000 });
  assert.match(await stateText(page), /done/i);
}

await check("homepage demo computes the Mandelbrot set at every detail level", async () => {
  const { page, errors } = await open("./", { width: 1440, height: 900 });
  await waitDone(page);
  for (const level of ["Low", "High", "Mid"]) {
    await page.locator("[data-panel] .seg button", { hasText: level }).click();
    await waitDone(page);
    assert.equal(await page.locator("[data-progress]").innerText(), "100%");
    const iterations = Number((await page.locator("[data-iters]").innerText()).replace(/,/g, ""));
    assert.ok(iterations > 1_000_000, `${level}: ${iterations} iterations`);
  }
  // The canvas holds the finished image: the palette's shades, not a blank.
  const colors = await page.evaluate(() => {
    const canvas = document.querySelector("[data-panel] canvas");
    const data = canvas.getContext("2d").getImageData(0, 0, canvas.width, canvas.height).data;
    const seen = new Set();
    for (let i = 0; i < data.length; i += 4) seen.add((data[i] << 16) | (data[i + 1] << 8) | data[i + 2]);
    return seen.size;
  });
  assert.ok(colors > 100, `only ${colors} colors on the canvas`);
  assert.match(await page.locator('[data-meta="engine"]').textContent(), /^quickjs-oxide \d+\.\d+\.\d+ · wasm32$/);
  assert.deepEqual(errors, []);
  await page.close();
});

await check("homepage Source view shows the files the demo runs and keeps the demo", async () => {
  const { page, errors } = await open("./", { width: 1440, height: 900 });
  await waitDone(page);
  const served = async (file) => (await (await fetch(new URL(file, base))).text());
  await page.locator("[data-view=source]").click();
  for (const [option, file] of [["js", "mandelbrot.js"], ["host", "worker.js"], ["paint", "paint.js"]]) {
    await page.locator("[data-panel] .source select").selectOption(option);
    await page.waitForFunction((head) => document.querySelector("[data-panel] .code").textContent.startsWith(head),
      (await served(file)).slice(0, 40));
    assert.equal(await page.locator("[data-panel] .code").textContent(), await served(file), file);
  }
  await page.locator("[data-view=demo]").click();
  assert.match(await stateText(page), /done/i, "switching views keeps the finished run");
  assert.deepEqual(errors, []);
  await page.close();
});

const slugs = DOC_NAV.flatMap((section) => section.items.map((item) => item.slug));
for (const viewport of [{ width: 1440, height: 900 }, { width: 390, height: 844 }]) {
  await check(`docs pages render at ${viewport.width}px`, async () => {
    for (const slug of slugs) {
      const { page, errors } = await open(`docs/${slug}/`, viewport);
      await page.waitForLoadState("networkidle");
      assert.ok((await page.locator("h1.doc-title").innerText()).trim(), `${slug}: no title`);
      assert.equal(await page.locator('.side-sec a[aria-current="page"]').count(), 1, `${slug}: sidebar`);
      const width = await page.evaluate(() => document.documentElement.scrollWidth);
      assert.ok(width <= viewport.width, `${slug}: page is ${width}px wide`);
      assert.deepEqual(errors, [], slug);
      await page.close();
    }
  });
}

await check("docs internal links and anchors resolve", async () => {
  const { page } = await open("docs/", { width: 1440, height: 900 });
  const seen = new Set();
  for (const slug of slugs) {
    await page.goto(new URL(`docs/${slug}/`, base).href);
    const links = await page.evaluate(() => [...document.querySelectorAll("a[href]")]
      .map((a) => a.href).filter((href) => href.startsWith(location.origin)));
    for (const link of links) {
      const url = new URL(link);
      const key = url.pathname + url.hash;
      if (seen.has(key)) continue;
      seen.add(key);
      const response = await fetch(url);
      assert.equal(response.status, 200, `${slug}: ${link}`);
      if (url.hash) {
        const html = await response.text();
        assert.ok(html.includes(`id="${decodeURIComponent(url.hash.slice(1))}"`), `${slug}: missing anchor ${link}`);
      }
    }
  }
  await page.close();
});

await browser.close();
server.close();
if (failures) {
  console.log(`${failures} check(s) failed`);
  process.exit(1);
}
console.log("site browser checks passed");
