#!/usr/bin/env node
// Verifies the live Cloudflare deployment serves this build: the homepage,
// the WebAssembly glue and module byte for byte, and the docs built from this
// commit. The deployment propagates through a CDN, so each check retries with
// backoff.
//
// Environment (set by .github/workflows/site.yml):
//   QUICKJS_OXIDE_PAGES_URL        the deployed site's base URL
//   QUICKJS_OXIDE_COMMIT           the commit the site was built from
//   QUICKJS_OXIDE_INDEX_SHA256     index.html
//   QUICKJS_OXIDE_GLUE_SHA256      pkg/quickjs_oxide_web.js
//   QUICKJS_OXIDE_WASM_SHA256      pkg/quickjs_oxide_web_bg.wasm
//   QUICKJS_OXIDE_LIVE_ATTEMPTS, QUICKJS_OXIDE_LIVE_RETRY_MS,
//   QUICKJS_OXIDE_LIVE_RETRY_MAX_MS, QUICKJS_OXIDE_LIVE_TIMEOUT_MS

import { createHash } from "node:crypto";
import process from "node:process";
import { setTimeout as wait } from "node:timers/promises";

function env(name, fallback) {
  const value = process.env[name] ?? fallback;
  if (value === undefined || value === "") throw new Error(`missing ${name}`);
  return value;
}

const base = new URL(env("QUICKJS_OXIDE_PAGES_URL").replace(/\/?$/, "/"));
const commit = env("QUICKJS_OXIDE_COMMIT");
const attempts = Number(env("QUICKJS_OXIDE_LIVE_ATTEMPTS", "15"));
const retryMs = Number(env("QUICKJS_OXIDE_LIVE_RETRY_MS", "2000"));
const retryMaxMs = Number(env("QUICKJS_OXIDE_LIVE_RETRY_MAX_MS", "10000"));
const timeoutMs = Number(env("QUICKJS_OXIDE_LIVE_TIMEOUT_MS", "15000"));

const CHECKS = [
  { path: "", sha256: env("QUICKJS_OXIDE_INDEX_SHA256"), type: "text/html" },
  { path: "pkg/quickjs_oxide_web.js", sha256: env("QUICKJS_OXIDE_GLUE_SHA256"), type: "javascript" },
  { path: "pkg/quickjs_oxide_web_bg.wasm", sha256: env("QUICKJS_OXIDE_WASM_SHA256"), type: "application/wasm" },
  { path: "docs/embed/", contains: `quickjs-oxide@${commit.slice(0, 8)}`, type: "text/html" },
];

async function verify({ path, sha256, contains, type }) {
  // A query string per attempt keeps a stale CDN copy from answering again.
  const url = new URL(path, base);
  for (let attempt = 1; ; attempt++) {
    url.searchParams.set("live-check", `${Date.now()}-${attempt}`);
    let problem;
    try {
      const response = await fetch(url, { signal: AbortSignal.timeout(timeoutMs), cache: "no-store" });
      const body = Buffer.from(await response.arrayBuffer());
      const contentType = response.headers.get("content-type") ?? "";
      if (!response.ok) problem = `HTTP ${response.status}`;
      else if (!contentType.includes(type)) problem = `content-type ${contentType}`;
      else if (sha256 && createHash("sha256").update(body).digest("hex") !== sha256) problem = "different bytes";
      else if (contains && !body.toString("utf8").includes(contains)) problem = `no "${contains}"`;
    } catch (error) {
      problem = String(error.message ?? error);
    }
    if (!problem) {
      console.log(`live ${path || "index.html"}: ok`);
      return;
    }
    if (attempt >= attempts) throw new Error(`live ${path || "index.html"}: ${problem} after ${attempt} attempts`);
    console.log(`live ${path || "index.html"}: ${problem}; retrying`);
    await wait(Math.min(retryMaxMs, retryMs * attempt));
  }
}

for (const check of CHECKS) await verify(check);
console.log(`the live site at ${base} serves ${commit}`);
