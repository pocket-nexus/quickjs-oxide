import { createRequire } from "node:module";
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { readFileSync, mkdirSync, writeFileSync, existsSync, statSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
const manifest = JSON.parse(readFileSync(path.join(here, "manifest.json"), "utf8"));
const args = Object.fromEntries(process.argv.slice(2).map(arg => {
  const index = arg.indexOf("=");
  if (index < 0) throw new Error(`expected --name=value: ${arg}`);
  return [arg.slice(2, index), arg.slice(index + 1)];
}));
if (!args.solid || !args.react || !args.vue || !args.output)
  throw new Error("required: --solid=DIR --react=DIR --vue=DIR --output=DIR");

function digest(data) { return createHash("sha256").update(data).digest("hex"); }
const identity = {};
for (const name of ["solid", "react", "vue"]) {
  const dir = path.resolve(args[name]);
  const actual = execFileSync("git", ["-C", dir, "rev-parse", "HEAD"], { encoding: "utf8" }).trim();
  if (actual !== manifest.apps[name].commit)
    throw new Error(`${name}: expected ${manifest.apps[name].commit}, got ${actual}`);
  const dirty = execFileSync("git", ["-C", dir, "status", "--porcelain", "--untracked-files=no"], { encoding: "utf8" });
  if (dirty) throw new Error(`${name}: tracked source is dirty`);
  identity[name] = { source: actual, lockfile_sha256: digest(readFileSync(path.join(dir, manifest.apps[name].lockfile))) };
}

const reactRequire = createRequire(path.join(path.resolve(args.react), "package.json"));
const solidRequire = createRequire(path.join(path.resolve(args.solid), "package.json"));
const vueRequire = createRequire(path.join(path.resolve(args.vue), "package.json"));
const esbuild = reactRequire("esbuild");
const babel = solidRequire("@babel/core");
const preset = solidRequire("babel-preset-solid");
const vueCompiler = vueRequire("@vue/compiler-sfc");
const solidDir = path.resolve(args.solid);
const solidSource = path.join(solidDir, "src") + path.sep;
const renderer = path.join(here, "solid_host.js");
const plugins = [{
  name: "oxide-solid-universal",
  setup(build) {
    build.onResolve({ filter: /^@oxide-app\// }, ({ path: specifier }) => {
      const base = path.join(solidDir, specifier.slice("@oxide-app/".length));
      const candidates = [base, `${base}.js`, path.join(base, "index.js")];
      const filename = candidates.find(candidate => existsSync(candidate) && statSync(candidate).isFile());
      if (!filename) throw new Error(`missing pinned app source: ${specifier}`);
      return { path: filename };
    });
    build.onResolve({ filter: /^@oxide-renderer$/ }, () => ({ path: renderer }));
    build.onLoad({ filter: /\.jsx?$/ }, ({ path: filename }) => {
      if (!filename.startsWith(solidSource)) return undefined;
      const source = readFileSync(filename, "utf8");
      const transformed = babel.transformSync(source, {
        filename, configFile: false, babelrc: false,
        presets: [[preset, { generate: "universal", moduleName: "@oxide-renderer" }]]
      });
      return { contents: transformed.code, loader: "js" };
    });
  }
}];

const output = path.resolve(args.output);
for (const dir of [path.resolve(args.solid), path.resolve(args.react), path.resolve(args.vue),
  path.resolve(here, "../../..")]) {
  if (output === dir || output.startsWith(dir + path.sep))
    throw new Error(`output must be outside source repositories: ${output}`);
}
if (existsSync(output)) throw new Error(`output directory already exists: ${output}`);
mkdirSync(output, { recursive: true });
const bundle = path.join(output, "solid-realworld.js");
await esbuild.build({
  entryPoints: [path.join(here, "solid_entry.js")], outfile: bundle,
  bundle: true, platform: "neutral", format: "iife", target: "es2020",
  mainFields: ["module", "main"],
  nodePaths: [path.join(solidDir, "node_modules")], plugins,
  define: { "process.env.NODE_ENV": '"production"' }
});
const vueDir = path.resolve(args.vue);
const vueSource = path.join(vueDir, "src");
const vuePlugins = [{
  name: "oxide-vue-headless",
  setup(build) {
    build.onResolve({ filter: /^dompurify$/ }, () => ({ path: path.join(here, "trusted_markdown.js") }));
    build.onResolve({ filter: /^\.\.?\// }, ({ path: specifier, importer }) => {
      if (!importer.startsWith(vueSource + path.sep)) return undefined;
      const base = path.resolve(path.dirname(importer), specifier);
      const candidates = [base, `${base}.js`, `${base}.vue`, path.join(base, "index.js")];
      const filename = candidates.find(candidate => existsSync(candidate) && statSync(candidate).isFile());
      return filename ? { path: filename } : undefined;
    });
    build.onResolve({ filter: /^(@oxide-vue\/|@\/)/ }, ({ path: specifier }) => {
      const relative = specifier.startsWith("@oxide-vue/")
        ? specifier.slice("@oxide-vue/".length) : specifier.slice(2);
      const base = path.join(vueSource, relative);
      const candidates = [base, `${base}.js`, `${base}.vue`, path.join(base, "index.js")];
      const filename = candidates.find(candidate => existsSync(candidate) && statSync(candidate).isFile());
      if (!filename) throw new Error(`missing pinned Vue source: ${specifier}`);
      return { path: filename };
    });
    build.onLoad({ filter: /\.vue$/ }, ({ path: filename }) => {
      const source = readFileSync(filename, "utf8");
      const { descriptor, errors } = vueCompiler.parse(source, { filename });
      if (errors.length) throw new Error(`${filename}: ${errors.join("; ")}`);
      const id = digest(filename).slice(0, 8);
      if (!descriptor.script && !descriptor.scriptSetup) {
        const template = vueCompiler.compileTemplate({ source: descriptor.template.content, filename, id });
        if (template.errors.length) throw new Error(`${filename}: ${template.errors.join("; ")}`);
        return { contents: `${template.code}\nexport default { render };`, loader: "js" };
      }
      const compiled = vueCompiler.compileScript(descriptor, { id, inlineTemplate: true });
      return { contents: compiled.content, loader: "js" };
    });
    build.onLoad({ filter: /\/src\/router\/index\.js$/ }, ({ path: filename }) => ({
      contents: readFileSync(filename, "utf8").replaceAll("createWebHistory", "createMemoryHistory"),
      loader: "js"
    }));
  }
}];
const vueBundle = path.join(output, "vue-realworld.js");
await esbuild.build({
  entryPoints: [path.join(here, "vue_entry.js")], outfile: vueBundle,
  bundle: true, platform: "neutral", format: "iife", target: "es2020",
  mainFields: ["module", "main"],
  nodePaths: [path.join(vueDir, "node_modules")], plugins: vuePlugins,
  define: { "process.env.NODE_ENV": '"production"', "import.meta.env.VITE_API_URL": "undefined" }
});
const reactDir = path.resolve(args.react);
const reactSource = path.join(reactDir, "src");
const reactPlugins = [{
  name: "oxide-react-headless",
  setup(build) {
    build.onResolve({ filter: /^react(?:\/.*)?$/ }, ({ path: specifier }) => ({
      path: reactRequire.resolve(specifier)
    }));
    build.onResolve({ filter: /^react-router$/ }, () => ({ path: path.join(here, "react_router_host.js") }));
    build.onResolve({ filter: /^(@oxide-react\/|~)/ }, ({ path: specifier }) => {
      const relative = specifier.startsWith("@oxide-react/")
        ? specifier.slice("@oxide-react/".length)
        : specifier.slice(1);
      const base = path.join(reactSource, relative);
      const candidates = [base, ...[".ts", ".tsx", ".js", ".jsx"].map(ext => base + ext),
        ...["index.ts", "index.tsx", "index.js"].map(name => path.join(base, name))];
      const filename = candidates.find(candidate => existsSync(candidate) && statSync(candidate).isFile());
      if (!filename) throw new Error(`missing pinned React source: ${specifier}`);
      return { path: filename };
    });
    build.onResolve({ filter: /\.css$/ }, () => ({ path: "empty-css", namespace: "oxide-css" }));
    build.onLoad({ filter: /.*/, namespace: "oxide-css" }, () => ({
      contents: "export default {}; export const spinner = 'spinner';", loader: "js"
    }));
  }
}];
const reactBundle = path.join(output, "react-realworld.js");
await esbuild.build({
  entryPoints: [path.join(here, "react_entry.js")], outfile: reactBundle,
  bundle: true, platform: "neutral", format: "iife", target: "es2020",
  mainFields: ["module", "main"], jsx: "automatic", minify: true,
  nodePaths: [path.join(reactDir, "node_modules")],
  plugins: reactPlugins, loader: { ".svg": "dataurl", ".png": "dataurl" },
  banner: { js: "globalThis.Intl ||= {}; Intl.DateTimeFormat ||= class { format() { return 'January 1, 2020'; } }; globalThis.console ||= { warn() {}, error() {}, log() {} }; globalThis.MessageChannel ||= class { constructor() { this.port1 = {}; this.port2 = {}; } }; globalThis.TextEncoder ||= class { encode(value) { return Uint8Array.from(String(value), char => char.charCodeAt(0)); } };" },
  define: { "process.env.NODE_ENV": '"production"' }
});
const receipt = {
  schema: manifest.schema, manifest_sha256: digest(readFileSync(path.join(here, "manifest.json"))),
  toolchain: { node: process.version, esbuild: esbuild.version, babel: babel.version,
    vue_compiler: vueCompiler.version, platform: process.platform, arch: process.arch },
  builder_sha256: digest(readFileSync(fileURLToPath(import.meta.url))),
  adapter_sha256: Object.fromEntries([
    "solid_entry.js", "solid_host.js", "vue_entry.js", "vue_host.js",
    "react_entry.js", "react_router_host.js", "trusted_markdown.js"
  ].map(name => [name, digest(readFileSync(path.join(here, name)))])),
  apps: identity, bundles: {
    solid: { file: bundle, sha256: digest(readFileSync(bundle)) },
    vue: { file: vueBundle, sha256: digest(readFileSync(vueBundle)) },
    react: { file: reactBundle, sha256: digest(readFileSync(reactBundle)) }
  }
};
writeFileSync(path.join(output, "receipt.json"), JSON.stringify(receipt, null, 2) + "\n");
console.log(JSON.stringify(receipt));
