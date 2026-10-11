#!/usr/bin/env node

// Admits every remaining positive Script-goal Test262 test that declares the
// dynamic-import feature. Each root's import graph is derived statically: a
// request is any literal specifier in an `import(...)` call or, inside an
// imported fixture module, in a static `import`/`export ... from` clause, and
// it joins the graph only when the requested file exists in the pinned suite.
// Requests that name a missing file stay outside the graph, so the Test262
// host rejects them, as those tests expect. A root whose own top-level code
// (outside comments, strings and templates) contains `import(` uses the
// initial-import-tree policy; a root that reaches `import(` only through
// runtime-compiled code uses runtime-compiled-import.

import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { existsSync, readFileSync, readdirSync } from "node:fs";
import { join, posix, resolve } from "node:path";

import {
  admissionRecord,
  assertAdmissionGroup,
  renderAdmissionRows,
} from "./test262-admission-data.mjs";

const root = resolve(import.meta.dirname, "../..");
const checkedSuite = join(root, "target/oracle/quickjs-2026-06-04/test262");
const checkedAdmissions = join(root, "dev-support/test262/admissions.tsv");
const checkedProfile = join(root, "compat/test262-oxide.conf");
const args = process.argv.slice(2);

let suite = checkedSuite;
let mode = "--check-current";
for (let index = 0; index < args.length; index += 1) {
  const argument = args[index];
  if (argument === "--suite") {
    const value = args[index + 1];
    assert(value && !value.startsWith("--"), "--suite requires a value");
    suite = resolve(value);
    index += 1;
  } else if (argument === "--admissions" || argument === "--check-current") {
    mode = argument;
  } else {
    assert.fail(`unknown option: ${argument}`);
  }
}
assert(existsSync(join(suite, "test")), `missing Test262 suite: ${suite}`);

const admissionGroup = "dynamic-import-b";
const sha256 = (value) => createHash("sha256").update(value).digest("hex");
const bytewise = (left, right) => Buffer.compare(Buffer.from(left), Buffer.from(right));
const source = (relativePath) => readFileSync(join(suite, relativePath), "utf8");
const frontmatter = (text) => text.match(/\/\*---[\s\S]*?---\*\/(?:\r?\n)?/)?.[0] ?? "";
const arrayField = (text, name) => {
  const match = text.match(new RegExp(`^${name}:\\s*\\[([^\\]]*)\\]`, "m"));
  return (
    match?.[1]
      .split(",")
      .map((value) => value.trim())
      .filter(Boolean) ?? []
  );
};

function metadata(relativePath) {
  const text = frontmatter(source(relativePath));
  if (!text) {
    return { includes: [], flags: [], features: [], negativePhase: "", negativeType: "" };
  }
  const negative = text.match(
    /^negative:\s*\n\s*phase:\s*([^\s]+)\s*\n\s*type:\s*([^\s]+)\s*$/m,
  );
  return {
    includes: arrayField(text, "includes"),
    // The Rust metadata parser stores flags in a BTreeSet.
    flags: arrayField(text, "flags").sort(),
    features: arrayField(text, "features"),
    negativePhase: negative?.[1] ?? "",
    negativeType: negative?.[2] ?? "",
  };
}

function testFiles(directory) {
  const files = [];
  for (const entry of readdirSync(join(suite, directory), { withFileTypes: true })) {
    const relativePath = `${directory}/${entry.name}`;
    if (entry.isDirectory()) files.push(...testFiles(relativePath));
    else if (entry.isFile() && entry.name.endsWith(".js") && !entry.name.includes("_FIXTURE")) {
      files.push(relativePath);
    }
  }
  return files;
}

function profileFeatures() {
  const lines = readFileSync(checkedProfile, "utf8").split("\n");
  const start = lines.indexOf("[features]");
  assert(start >= 0, "profile has no [features] section");
  const features = new Set();
  for (const line of lines.slice(start + 1)) {
    if (line.startsWith("[")) break;
    if (line) features.add(line);
  }
  return features;
}

function otherGroupRoots() {
  const roots = new Set();
  for (const line of readFileSync(checkedAdmissions, "utf8").trimEnd().split("\n").slice(1)) {
    const [kind, group, path] = line.split("\t");
    if (group !== admissionGroup && /^(?:dynamic-import-root|graph-root|module)$/.test(kind)) {
      roots.add(path);
    }
  }
  return roots;
}

const declaredFeatures = profileFeatures();
const admittedElsewhere = otherGroupRoots();
const roots = testFiles("test")
  .filter((relativePath) => {
    const shape = metadata(relativePath);
    return (
      shape.features.includes("dynamic-import") &&
      !shape.flags.includes("module") &&
      shape.negativePhase === "" &&
      shape.features.every(
        (feature) => feature === "dynamic-import" || declaredFeatures.has(feature),
      ) &&
      !admittedElsewhere.has(relativePath)
    );
  })
  .sort(bytewise);

/// Source text with comments, string literals and template literals blanked,
/// so `import(` matches only code the parser compiles up front.
function codeOnly(text) {
  let output = "";
  let index = 0;
  while (index < text.length) {
    const rest = text.slice(index);
    if (rest.startsWith("//")) {
      const end = text.indexOf("\n", index);
      index = end === -1 ? text.length : end;
    } else if (rest.startsWith("/*")) {
      const end = text.indexOf("*/", index + 2);
      index = end === -1 ? text.length : end + 2;
    } else if (rest[0] === '"' || rest[0] === "'" || rest[0] === "`") {
      const quote = rest[0];
      index += 1;
      while (index < text.length && text[index] !== quote) {
        index += text[index] === "\\" ? 2 : 1;
      }
      index += 1;
      output += " ";
    } else {
      output += rest[0];
      index += 1;
    }
  }
  return output;
}

const requestPatterns = {
  script: [/\bimport\s*\(\s*(["'])([^"']+)\1/g],
  module: [
    /\bimport\s*\(\s*(["'])([^"']+)\1/g,
    /\bimport\s+(?!\()(?:(?:[\w$*{},\s]+)\s+from\s+)?(["'])([^"']+)\1/g,
    /\bexport\s+(?:\*(?:\s+as\s+[\w$]+)?|\{[^}]*\})\s*from\s*(["'])([^"']+)\1/g,
  ],
};

function requests(relativePath, goal) {
  const body = source(relativePath).replace(/\/\*---[\s\S]*?---\*\//, "");
  const matches = [];
  for (const pattern of requestPatterns[goal]) {
    for (const match of body.matchAll(pattern)) {
      matches.push({ offset: match.index, specifier: match[2] });
    }
  }
  matches.sort((left, right) => left.offset - right.offset);
  const edges = [];
  for (const { specifier } of matches) {
    if (!specifier.startsWith("./") && !specifier.startsWith("../")) continue;
    const normalized = posix.normalize(posix.join(posix.dirname(relativePath), specifier));
    assert(normalized.startsWith("test/"), `${relativePath}: request escapes the suite`);
    if (!existsSync(join(suite, normalized))) continue;
    if (!edges.some((edge) => edge.specifier === specifier)) {
      edges.push({ specifier, normalized });
    }
  }
  return edges;
}

const fileEdges = new Map();
const pending = roots.map((relativePath) => [relativePath, "script"]);
while (pending.length !== 0) {
  const [relativePath, goal] = pending.pop();
  if (fileEdges.has(relativePath)) continue;
  const edges = requests(relativePath, goal);
  fileEdges.set(relativePath, edges);
  for (const edge of edges) pending.push([edge.normalized, "module"]);
}

function closureSize(rootPath) {
  const visited = new Set();
  const stack = [rootPath];
  while (stack.length !== 0) {
    const relativePath = stack.pop();
    if (visited.has(relativePath)) continue;
    visited.add(relativePath);
    for (const edge of fileEdges.get(relativePath)) stack.push(edge.normalized);
  }
  return visited.size;
}

const policy = (rootPath) =>
  /\bimport\s*\(/.test(codeOnly(source(rootPath).replace(/\/\*---[\s\S]*?---\*\//, "")))
    ? "initial-import-tree"
    : "runtime-compiled-import";

const sources = [...fileEdges.keys()].sort(bytewise);
const admissionRecords = [
  ...sources.map((relativePath) => {
    const shape = metadata(relativePath);
    return admissionRecord({
      kind: "graph-file",
      group: admissionGroup,
      path: relativePath,
      source_sha256: sha256(source(relativePath)),
      includes: shape.includes,
      flags: shape.flags,
      features: shape.features,
      negative_phase: shape.negativePhase,
      negative_type: shape.negativeType,
    });
  }),
  ...sources.flatMap((relativePath) =>
    fileEdges.get(relativePath).map((edge, requestIndex) =>
      admissionRecord({
        kind: "graph-request",
        group: admissionGroup,
        path: relativePath,
        request_index: requestIndex,
        specifier: edge.specifier,
        normalized_path: edge.normalized,
      }),
    ),
  ),
  ...roots.map((rootPath) =>
    admissionRecord({
      kind: "dynamic-import-root",
      group: admissionGroup,
      path: rootPath,
      closure_file_count: closureSize(rootPath),
      priority: 0,
      policy: policy(rootPath),
    }),
  ),
];

const edgeCount = [...fileEdges.values()].reduce((count, edges) => count + edges.length, 0);
const policyCounts = roots.reduce((counts, rootPath) => {
  counts[policy(rootPath)] = (counts[policy(rootPath)] ?? 0) + 1;
  return counts;
}, {});
const summary =
  `roots=${roots.length} files=${sources.length} edges=${edgeCount} ` +
  `initial=${policyCounts["initial-import-tree"] ?? 0} ` +
  `runtime-compiled=${policyCounts["runtime-compiled-import"] ?? 0}`;

if (mode === "--admissions") {
  process.stdout.write(renderAdmissionRows(admissionRecords));
} else {
  assert.equal(
    summary,
    "roots=438 files=481 edges=418 initial=437 runtime-compiled=1",
    "dynamic-import-b cohort drifted",
  );
  assertAdmissionGroup(checkedAdmissions, admissionGroup, admissionRecords);
  console.log(`dynamic-import-b admissions authenticated: ${summary}`);
}
