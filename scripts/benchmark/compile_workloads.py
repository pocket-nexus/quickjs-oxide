#!/usr/bin/env python3
"""Generate project-authored Script-goal sources for front-end compile timing.

These files are diagnostics for parse/compile throughput, not runtime benchmarks
and not upstream scores. Content is deterministic so hashes are reproducible;
each repetition is a block so repeated declarations stay legal, and `{i}` names
are suffixed per repetition so the corpus is not one identifier repeated
thousands of times. When Node is available every generated file is
independently parse-checked.
"""
import argparse
import json
import shutil
import subprocess
from pathlib import Path

from run import digest

CASES = ("syntax-mixed", "functions", "expressions")

SYNTAX_BLOCK = """{
  class Box{i} {
    #value = 1;
    static count = 0;
    static { Box{i}.count = 1; }
    constructor(value = 0) { this.#value = value; }
    get value() { return this.#value; }
    set value(next) { this.#value = next; }
    *entries() { yield this.#value; yield* [1, 2].values(); }
    async load() { return await Promise.resolve(this.#value); }
  }
  const [first{i} = 1, ...rest{i}] = [1, 2, 3];
  const { a: renamed{i} = first{i}, b: { c } = { c: 4 }, ...others{i} } = { a: 1, b: { c: 2 }, d: 3 };
  const merged{i} = { ...others{i}, renamed{i}, c, [`key${c}`]: rest{i}.length };
  const template{i} = `head ${first{i} + 1} ${`nested ${c}`} tail`;
  const pattern{i} = /(?<word>\\p{ID_Start}+)(?:\\s+)(?<rest>.*)/du;
  const matched{i} = pattern{i}.exec(template{i})?.groups?.word ?? "fallback";
  label{i}: for (let i = 0; i < 2; i++) {
    switch (i) {
      case 0: continue label{i};
      case 1: break label{i};
      default: break;
    }
  }
  const arrow{i} = (x, y = 2, ...tail) => ({ x, y, tail });
  const asyncArrow{i} = async ({ value = 0 } = {}) => await value;
  function* generate{i}(n) {
    try { for (let i = 0; i < n; i++) yield i; }
    finally { Box{i}.count += 1; }
  }
  async function consume{i}(source) {
    for await (const item of source) { if (item) break; }
    return source?.[0] ?? null;
  }
  const nested{i} = (function outer{i}() { return function inner{i}() { return arrow{i}(1)?.x; }; })();
  void merged{i}, matched{i}, generate{i}, consume{i}, asyncArrow{i}, nested{i};
}"""

FUNCTION_BLOCK = """{
  function computeAlpha{i}(alpha, beta = alpha, ...gamma) {
    let accumulator = alpha + beta;
    const offset = gamma.length;
    for (let index = 0; index < offset; index++) {
      accumulator += gamma[index] * (index + 1);
    }
    return accumulator;
  }
  function computeBeta{i}({ delta = 0, epsilon = 1 } = {}, [zeta = 2] = []) {
    const theta = delta + epsilon + zeta;
    return theta * 2 - 1;
  }
  const computeGamma{i} = function namedGamma{i}(iota, kappa) {
    return iota < kappa ? computeAlpha{i}(iota, kappa) : computeBeta{i}({ delta: iota });
  };
  const computeDelta{i} = (lambda = 0, mu = 1) => lambda * mu + computeGamma{i}(lambda, mu);
  function* computeEpsilon{i}(nu) { yield computeDelta{i}(nu, nu + 1); yield* [nu].values(); }
  async function computeZeta{i}(xi) { return await computeDelta{i}(xi, xi + 2); }
  class Computer{i} {
    static #instances = 0;
    #state = computeBeta{i}();
    constructor() { Computer{i}.#instances += 1; }
    method(omicron) { return computeDelta{i}(omicron, this.#state); }
    static get instances() { return Computer{i}.#instances; }
  }
  void computeAlpha{i}, computeBeta{i}, computeGamma{i}, computeDelta{i}, computeEpsilon{i}, computeZeta{i}, Computer{i};
}"""

EXPRESSION_BLOCK = """{
  let accumulator{i} = 0;
  accumulator{i} += ((1 + 2) * 3 - 4) / 5 % 6;
  accumulator{i} = accumulator{i} ** 2 + Math.max(accumulator{i}, 0) - Math.min(accumulator{i}, 1);
  accumulator{i} ||= 7; accumulator{i} &&= 8; accumulator{i} ??= 9;
  accumulator{i} = (accumulator{i} > 10 ? accumulator{i} - 10 : accumulator{i} + 10) << 1;
  accumulator{i} = (accumulator{i} | 3) & 7 ^ 1;
  accumulator{i} = ~accumulator{i} + (accumulator{i} >>> 1);
  accumulator{i} = "value" + accumulator{i} + true + null + undefined;
  accumulator{i} = typeof accumulator{i} === "string" ? accumulator{i}.length : 0;
  const members{i} = { first: { second: { third: [1, 2, 3] } } };
  accumulator{i} += members{i}?.first?.second?.third?.[0] ?? 0;
  accumulator{i} += members{i}.first["second"].third.at(-1) ?? 0;
  accumulator{i} += (() => 1)() + (function () { return 2; })() + (async () => 3).constructor.name.length;
  accumulator{i} += (function () { return new.target === undefined ? 0 : 1; })();
  accumulator{i} = `${accumulator{i}}${1 + 1}`.length;
  accumulator{i} += /[a-z]+/i.test("AbC") ? 1 : 0;
  accumulator{i} += (() => { try { throw new Error("x"); } catch { return 1; } finally { accumulator{i} += 0; } })();
  accumulator{i} += [1, 2, 3].map((value) => value * 2).filter((value) => value > 2).length;
  accumulator{i} += Object.entries({ a: 1 }).length + Object.keys({ b: 2 }).length;
  accumulator{i} += (0, eval)("1").length;
  accumulator{i} += (function () { return arguments.length; })(1, 2, 3);
  void accumulator{i};
}"""

BLOCKS = {"syntax-mixed": SYNTAX_BLOCK, "functions": FUNCTION_BLOCK, "expressions": EXPRESSION_BLOCK}


def generate(case, size):
    if case not in BLOCKS or size < 1:
        raise ValueError("known case and positive size required")
    block = BLOCKS[case]
    repetitions = max(1, -(-size // len(block)))
    source = "// Project-authored compile workload; generated by scripts/benchmark/compile_workloads.py.\n"
    for index in range(repetitions):
        source += block.replace("{i}", str(index))
    return source + "\n"


def command_version(node):
    if node is None:
        return None
    result = subprocess.run([node, "--version"], capture_output=True, text=True, check=True)
    return result.stdout.strip()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True, help="new workload directory")
    parser.add_argument("--sizes", type=int, nargs="+", default=[65536, 524288, 4194304])
    parser.add_argument("--case", action="append", dest="cases", choices=CASES)
    args = parser.parse_args()
    if any(size < 1 for size in args.sizes) or len(set(args.sizes)) != len(args.sizes):
        parser.error("sizes must be unique positive byte counts")
    output = args.output.resolve()
    if output.exists():
        parser.error(f"output directory already exists: {output}")
    output.mkdir(parents=True)
    node = shutil.which("node")
    workloads = []
    for case in args.cases or CASES:
        for size in args.sizes:
            path = output / f"{case}-{size}.js"
            path.write_text(generate(case, size))
            if node is not None:
                subprocess.run([node, "--check", str(path)], check=True)
            workloads.append({"case": path.stem, "path": str(path), "bytes": path.stat().st_size,
                              "sha256": digest(path), "target_bytes": size, "generator": case})
    manifest = {"schema": "oxide-compile-corpus-v1", "generator_sha256": digest(__file__),
                "node_check": command_version(node), "workloads": workloads}
    (output / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    for workload in workloads:
        print(f"{workload['case']} {workload['bytes']}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
