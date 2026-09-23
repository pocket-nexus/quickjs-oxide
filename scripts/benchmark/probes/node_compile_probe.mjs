// V8 (Node) front-end probe for the cross-engine compile matrix.
//
// `compile` times `new vm.Script` with an eager compilation flag supplied by the
// runner (`--no-lazy`). `parse` times the same constructor under V8's
// `--parse-only` flag, also supplied by the runner. Source I/O and Node startup
// stay outside the timed interval. Output is exactly one line:
// parse_ns:<integer> or compile_ns:<integer>.
import { readFileSync } from "node:fs";
import { hrtime, argv } from "node:process";
import vm from "node:vm";

if (argv.length === 3 && argv[2] === "--version") {
  console.log("node-compile-probe 1");
  process.exit(0);
}

const [, , metric, file] = argv;
if ((metric !== "parse" && metric !== "compile") || !file) {
  console.error("usage: node-compile-probe <parse|compile> FILE");
  process.exit(2);
}

const source = readFileSync(file, "utf8");
const started = hrtime.bigint();
const script = new vm.Script(source, { filename: file });
const elapsed = hrtime.bigint() - started;
if (script === undefined) {
  console.error("vm.Script returned no script");
  process.exit(1);
}
console.log(`${metric}_ns:${elapsed}`);
