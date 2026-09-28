# Pinned RealWorld application holdouts

This phase uses the Vue, React, and Solid implementations of the same RealWorld
application. `manifest.json` pins source commits, dependency locks, seed size,
and the seven semantic phases. These are **holdouts** for VM optimization;
their output and work must be frozen before candidate measurements. If a result
drives an engine change, reclassify that case as development data and introduce
a new holdout.

Clone each repository listed in the manifest outside this repository, check out
its exact commit, then install its locked dependencies (`bun install
--frozen-lockfile` for Vue, `corepack yarn install --frozen-lockfile
--ignore-scripts` for React, and `npm ci` for Solid). With three checkout paths:

```sh
node scripts/benchmark/realworld/build.mjs \
  --vue=/absolute/vue --react=/absolute/react --solid=/absolute/solid \
  --output=/absolute/new-bundle-directory
python3 scripts/benchmark/realworld/prepare.py \
  --receipt=/absolute/new-bundle-directory/receipt.json \
  --build-receipt=/absolute/baseline/qjs.build.json \
  --engine=/absolute/baseline/qjs \
  --output=/absolute/new-bundle-directory/workloads.json
python3 scripts/benchmark/fixed.py \
  --manifest=/absolute/new-bundle-directory/workloads.json \
  --engine baseline=/absolute/baseline/qjs \
  --engine candidate=/absolute/candidate/qjs \
  --order abba-baab --repeat 4 \
  --output=/absolute/new-results-directory
```

The adapters supply an in-memory RealWorld API and headless render targets.
Solid and Vue mount their actual app components and stores. React renders its
actual page components and state helpers through its browser-capable server
renderer, because the pinned `react-test-renderer` bundle fails publication on
the frozen Oxide baseline. Each phase records a tree or HTML digest; the
admission step requires the same exact output from Node and the frozen Oxide
binary. CSS and browser-only markdown sanitization are outside the measured
guest behavior. The article bodies are trusted, fixed plain text.

The React adapter supplies route data and models favorite/comment transitions;
it does not execute that app's router loader, network layer, or form actions.
Those paths are covered by the separate Vue/Solid app flows and by focused
semantic tests. Do not claim a React network/action improvement from this case.

These are application transfer workloads, not scores comparing frameworks.
The fixed runner measures whole processes, including startup and compilation.
Do not repeatedly execute the mutating scripts in one process without a fresh
application state and cache-reset protocol. Record bundle hashes, exact output,
binary receipts, and complete balanced process blocks. A zero region hit count
is valid negative evidence, not grounds to edit the holdout.
