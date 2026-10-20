#!/usr/bin/env node
// Build the static site into OUT_DIR: the homepage from apps/web/site and one
// docs page per entry of apps/web/docs/nav.mjs, rendered from Markdown.
//
//   node scripts/web/build-site.mjs OUT_DIR
//
// DOCS_PREVIEW=1 renders a placeholder for a page whose Markdown is missing.
//
// Docs pages share the homepage's header and footer (read from its
// index.html) and its stylesheet. A Markdown line of the form
//
//   <!-- include: examples/embed.rs -->
//
// is replaced by that file as a code block, so examples on the page are the
// files the test suite compiles and runs. `include: FILE#NAME` takes only the
// lines between `// docs:start NAME` and `// docs:end NAME`.
//
// The homepage's Test262 figures come from the frozen Test262 vector. Every
// script, stylesheet and WebAssembly file is referenced with a `?v=` content
// digest, so a page never pairs with files from another deployment while the
// Pages CDN propagates (pkg/ is versioned when the WebAssembly build put it
// into OUT_DIR first).

import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { cpSync, existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { Marked, Renderer } from "marked";
import { createHighlighter } from "shiki";
import { DOC_NAV } from "../../apps/web/docs/nav.mjs";
import { readCurrentTest262Metrics } from "../test262/current-test262-metrics.mjs";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const SITE = path.join(ROOT, "apps/web/site");
const DOCS = path.join(ROOT, "apps/web/docs");
const GH = "https://github.com/pocket-nexus/quickjs-oxide";

const [outArg] = process.argv.slice(2);
if (!outArg || process.argv.length !== 3) {
  console.error("usage: build-site.mjs OUT_DIR");
  process.exit(2);
}
const OUT = path.resolve(outArg);

// The commit the pages link their sources to: CI's, or the checkout's.
const commit = /^[0-9a-f]{40}$/.test(process.env.QUICKJS_OXIDE_COMMIT ?? "")
  ? process.env.QUICKJS_OXIDE_COMMIT
  : execFileSync("git", ["rev-parse", "HEAD"], { cwd: ROOT, encoding: "utf8" }).trim();
const shortCommit = commit.slice(0, 8);

// Code colors from the site palette.
const THEME = {
  name: "oxide",
  type: "dark",
  colors: { "editor.background": "#120c21", "editor.foreground": "#e8e1ff" },
  tokenColors: [
    { scope: ["comment", "punctuation.definition.comment"], settings: { foreground: "#8a7fae", fontStyle: "italic" } },
    { scope: ["string", "string.quoted", "constant.character"], settings: { foreground: "#ffd23f" } },
    { scope: ["constant.numeric", "constant.language", "constant.other"], settings: { foreground: "#ffa94f" } },
    { scope: ["keyword", "storage", "storage.type", "storage.modifier", "keyword.control"], settings: { foreground: "#ff5f9e" } },
    { scope: ["entity.name.function", "support.function", "meta.function-call"], settings: { foreground: "#3fd0e8" } },
    { scope: ["entity.name.type", "support.type", "entity.name.class", "entity.name.namespace"], settings: { foreground: "#a98bff" } },
    { scope: ["variable.parameter"], settings: { foreground: "#f3d9ff" } },
    { scope: ["entity.name.tag", "entity.other.attribute-name", "support.type.property-name"], settings: { foreground: "#3fd0e8" } },
    { scope: ["punctuation", "keyword.operator"], settings: { foreground: "#c9bfe8" } },
  ],
};
const LANGS = ["rust", "javascript", "toml", "shellscript", "json", "text"];
const LANG_ALIAS = { js: "javascript", mjs: "javascript", rs: "rust", sh: "shellscript", console: "shellscript" };
const highlighter = await createHighlighter({ themes: [THEME], langs: LANGS });

function escapeHtml(text) {
  return text.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");
}

function slugify(text) {
  return text.toLowerCase().replace(/<[^>]+>/g, "").replace(/&#?\w+;/g, "")
    .replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");
}

// ---- includes ----

function expandIncludes(markdown, file) {
  return markdown.replace(/^<!-- include: ([^#\s]+)(?:#([\w-]+))? -->$/gm, (_, rel, region) => {
    const source = path.join(ROOT, rel);
    if (!existsSync(source)) throw new Error(`${file}: missing include ${rel}`);
    let text = readFileSync(source, "utf8");
    if (region) {
      const start = text.indexOf(`// docs:start ${region}\n`);
      const end = text.indexOf(`// docs:end ${region}`);
      if (start < 0 || end < start) throw new Error(`${file}: no region ${region} in ${rel}`);
      text = dedent(text.slice(text.indexOf("\n", start) + 1, end).replace(/[ \t]*$/, ""));
    }
    text = text.replace(/^[ \t]*\/\/ docs:(start|end) [\w-]+\n/gm, "").trimEnd();
    const ext = path.extname(rel).slice(1);
    return "```" + (LANG_ALIAS[ext] ?? ext) + ` file=${rel}\n${text}\n` + "```";
  });
}

function dedent(text) {
  const lines = text.split("\n");
  const indent = Math.min(...lines.filter((l) => l.trim()).map((l) => l.match(/^ */)[0].length));
  return lines.map((l) => l.slice(indent)).join("\n");
}

// ---- Markdown ----

function renderMarkdown(markdown, slug) {
  const toc = [];
  let title = null;
  const marked = new Marked({
    gfm: true,
    renderer: {
      heading({ tokens, depth }) {
        const html = this.parser.parseInline(tokens);
        if (depth === 1) {
          title = html;
          return "";
        }
        const id = slugify(html);
        if (depth === 2 || depth === 3) toc.push({ depth, id, html });
        const mark = depth === 2 ? '<span class="star" aria-hidden="true"></span>' : "";
        return `<h${depth} id="${id}">${mark}<a href="#${id}">${html}</a></h${depth}>\n`;
      },
      code({ text, lang }) {
        const [name, ...meta] = (lang ?? "text").split(/\s+/);
        const language = LANG_ALIAS[name] ?? name;
        if (!LANGS.includes(language)) throw new Error(`${slug}: unknown code language ${name}`);
        const file = meta.find((m) => m.startsWith("file="))?.slice(5);
        const label = file
          ? `<a class="code-file" href="${GH}/blob/${commit}/${file}">${escapeHtml(file)}</a>`
          : "";
        const tag = language === "text" ? "" : `<span class="code-lang">${name === "sh" || name === "console" ? "shell" : name}</span>`;
        const body = highlighter.codeToHtml(text, { lang: language, theme: "oxide" });
        return `<figure class="code">${label || tag ? `<figcaption>${label}${tag}</figcaption>` : ""}${body}</figure>\n`;
      },
      table(token) {
        return `<div class="table-wrap">${Renderer.prototype.table.call(this, token)}</div>\n`;
      },
      link({ href, title: linkTitle, tokens }) {
        const text = this.parser.parseInline(tokens);
        const target = href.replace(/^\.\/([\w-]+)\.md(#.*)?$/, "../$1/$2");
        const external = /^https?:/.test(target) && !target.startsWith(GH + "/blob/" + commit);
        return `<a href="${escapeHtml(target)}"${linkTitle ? ` title="${escapeHtml(linkTitle)}"` : ""}${external ? ' rel="noreferrer"' : ""}>${text}${external ? '<span class="ext" aria-hidden="true">↗</span>' : ""}</a>`;
      },
    },
  });
  const html = marked.parse(markdown);
  if (!title) throw new Error(`${slug}: the page needs a "# Title" line`);
  return { title, html, toc };
}

// ---- page shell ----

const homepage = readFileSync(path.join(SITE, "index.html"), "utf8");
function pick(re, what) {
  const match = homepage.match(re);
  if (!match) throw new Error(`apps/web/site/index.html: cannot find the ${what}`);
  return match[0];
}
const HEAD_LINKS = pick(/<link rel="preconnect"[\s\S]*?<link rel="stylesheet" href="site.css">/, "stylesheet links");
const HEADER = pick(/<nav class="top">[\s\S]*?<\/nav>\s*<div class="rainbow dots"><\/div>/, "header");
const FOOTER = pick(/<footer class="site">[\s\S]*?<\/footer>/, "footer");

// Docs pages live two levels down (docs/<slug>/), so the shell's relative
// links take a prefix.
function relocate(html, prefix) {
  return html.replace(/(href|src)="(?![a-z]+:|#|\/)([^"]*)"/g, (_, attr, url) => `${attr}="${prefix}${url}"`);
}

const pages = DOC_NAV.flatMap((section) => section.items.map((item) => ({ ...item, section: section.title })));

function sidebar(active) {
  const sections = DOC_NAV.map((section, i) => `
      <div class="side-sec s${i}">
        <p>${escapeHtml(section.title)}</p>
        ${section.items.map((item) =>
          `<a href="../${item.slug}/"${item.slug === active ? ' aria-current="page"' : ""}>${escapeHtml(item.title)}</a>`).join("\n        ")}
      </div>`).join("");
  return `${sections}
      <div class="side-extra">
        <a class="try" href="../../">Try the demo <span aria-hidden="true">→</span></a>
      </div>`;
}

function docPage(page, index, { title, html, toc }) {
  const prev = pages[index - 1], next = pages[index + 1];
  const plainTitle = title.replace(/<[^>]+>/g, "");
  const tocHtml = toc.length
    ? `<p>On this page</p>${toc.map((t) => `<a class="d${t.depth}" href="#${t.id}">${t.html}</a>`).join("")}`
    : "";
  const header = relocate(HEADER, "../../").replace('class="navlink hide-sm"', 'class="navlink hide-sm on"');
  return `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover">
<title>${escapeHtml(plainTitle)} · quickjs-oxide docs</title>
<meta name="color-scheme" content="dark">
<meta name="theme-color" content="#171226">
<link rel="icon" href="../../favicon.svg" type="image/svg+xml">
${relocate(HEAD_LINKS, "../../")}
<link rel="stylesheet" href="../../docs.css">
</head>
<body class="docs">
${header}
<div class="wrap docs-grid">
  <aside class="side" aria-label="Docs">
    <details class="side-menu"><summary>${escapeHtml(page.section)} · ${escapeHtml(page.title)}</summary>${sidebar(page.slug)}
    </details>
  </aside>
  <main class="article">
    <div class="chips"><span class="chip">${escapeHtml(page.section)}</span><a class="chip" href="${GH}/blob/${commit}/apps/web/docs/${page.slug}.md">Source · quickjs-oxide@${shortCommit}</a></div>
    <h1 class="px px-shadow doc-title">${title}</h1>
    ${html}
    <nav class="pager" aria-label="Pages">
      ${prev ? `<a class="prev" href="../${prev.slug}/"><span>← Previous</span><b>${escapeHtml(prev.title)}</b></a>` : "<span></span>"}
      ${next ? `<a class="next" href="../${next.slug}/"><span>Next →</span><b>${escapeHtml(next.title)}</b></a>` : "<span></span>"}
    </nav>
  </main>
  <nav class="toc" aria-label="On this page">${tocHtml}</nav>
</div>
${relocate(FOOTER, "../../").replace("</footer>", `<p class="wrap built">built from quickjs-oxide@${shortCommit}</p></footer>`)}
<script>
// Keep the sidebar open on wide screens; it is a disclosure on phones.
const menu = document.querySelector(".side-menu");
const wide = matchMedia("(min-width: 900px)");
const sync = () => { menu.open = wide.matches; };
wide.addEventListener("change", sync); sync();
</script>
</body>
</html>
`;
}

// ---- build ----

mkdirSync(OUT, { recursive: true });
rmSync(path.join(OUT, "docs"), { recursive: true, force: true });
cpSync(SITE, OUT, { recursive: true });

for (const [index, page] of pages.entries()) {
  const file = path.join(DOCS, `${page.slug}.md`);
  let markdown;
  if (existsSync(file)) markdown = readFileSync(file, "utf8");
  else if (process.env.DOCS_PREVIEW) markdown = `# ${page.title}\n\nThis page is not written yet.\n`;
  else throw new Error(`missing docs page apps/web/docs/${page.slug}.md`);
  const rendered = renderMarkdown(expandIncludes(markdown, file), page.slug);
  const dir = path.join(OUT, "docs", page.slug);
  mkdirSync(dir, { recursive: true });
  writeFileSync(path.join(dir, "index.html"), docPage(page, index, rendered));
}
// ---- Test262 figures ----

const metrics = readCurrentTest262Metrics();
replaceIn("index.html", [
  ["@@TEST262_PERCENT@@", Number(metrics.quickJsScopePercent).toFixed(2)],
  ["@@TEST262_PASSES@@", metrics.fullPasses.toLocaleString("en-US")],
  ["@@TEST262_SCOPE@@", metrics.quickJsScope.toLocaleString("en-US")],
]);

// ---- content digests ----

const docPages = pages.map((page) => `docs/${page.slug}/index.html`);
const VERSIONED = [
  ["pkg/quickjs_oxide_web.js", { "worker.js": ['"./pkg/quickjs_oxide_web.js"'] }],
  ["pkg/quickjs_oxide_web_bg.wasm", { "worker.js": ['"./pkg/quickjs_oxide_web_bg.wasm"'] }],
  ["mandelbrot.js", { "worker.js": ['"./mandelbrot.js"'], "home.js": ['"mandelbrot.js"'] }],
  ["worker.js", { "home.js": ['"worker.js"'] }],
  ["paint.js", { "home.js": ['"paint.js"'], "index.html": ['"paint.js"'] }],
  ["home.js", { "index.html": ['"home.js"'] }],
  ["site.css", { "index.html": ['"site.css"'], ...Object.fromEntries(docPages.map((p) => [p, ['"../../site.css"']])) }],
  ["docs.css", Object.fromEntries(docPages.map((p) => [p, ['"../../docs.css"']]))],
];
for (const [file, referrers] of VERSIONED) {
  if (file.startsWith("pkg/") && !existsSync(path.join(OUT, file))) continue;
  const digest = createHash("sha256").update(readFileSync(path.join(OUT, file))).digest("hex").slice(0, 16);
  for (const [referrer, literals] of Object.entries(referrers)) {
    replaceIn(referrer, literals.map((literal) => [literal, `${literal.slice(0, -1)}?v=${digest}"`]));
  }
}

function replaceIn(file, pairs) {
  const target = path.join(OUT, file);
  let text = readFileSync(target, "utf8");
  for (const [from, to] of pairs) {
    if (!text.includes(from)) throw new Error(`${file}: expected ${from}`);
    text = text.replaceAll(from, to);
  }
  writeFileSync(target, text);
}

writeFileSync(path.join(OUT, "docs", "index.html"), `<!doctype html>
<meta charset="utf-8"><title>quickjs-oxide docs</title>
<meta http-equiv="refresh" content="0; url=${pages[0].slug}/">
<link rel="canonical" href="${pages[0].slug}/">
<a href="${pages[0].slug}/">Docs</a>
`);
console.log(`built the homepage and ${pages.length} docs pages into ${path.relative(ROOT, OUT) || "."}`);
