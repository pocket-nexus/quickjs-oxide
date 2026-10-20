#!/usr/bin/env node
// Writes the pages GitHub Pages serves at the site's former address: each
// sends its path under OLD_PATH to the same path on the site at NEW_URL.
// index.html answers the old root and 404.html every other old path.
//
//   node scripts/web/build-redirect.mjs OUT_DIR NEW_URL OLD_PATH

import { mkdirSync, writeFileSync } from "node:fs";
import path from "node:path";
import process from "node:process";

const [out, newUrl, oldPath] = process.argv.slice(2);
if (!out || !newUrl || !oldPath) {
  console.error("usage: build-redirect.mjs OUT_DIR NEW_URL OLD_PATH");
  process.exit(2);
}
const site = new URL(newUrl).href;
const html = (text) => text.replace(/[&<>"]/g, (c) => `&#${c.charCodeAt(0)};`);

const page = `<!doctype html>
<meta charset="utf-8">
<title>quickjs-oxide has moved</title>
<link rel="canonical" href="${html(site)}">
<script>
  const old = ${JSON.stringify(oldPath.replace(/\/$/, ""))};
  const rest = location.pathname.startsWith(old + "/") ? location.pathname.slice(old.length + 1) : "";
  location.replace(new URL(rest + location.search + location.hash, ${JSON.stringify(site)}));
</script>
<noscript><meta http-equiv="refresh" content="0; url=${html(site)}"></noscript>
<p>quickjs-oxide has moved to <a href="${html(site)}">${html(site)}</a>.</p>
`;

mkdirSync(out, { recursive: true });
for (const name of ["index.html", "404.html"]) writeFileSync(path.join(out, name), page);
console.log(`wrote the redirect to ${site} into ${out}`);
