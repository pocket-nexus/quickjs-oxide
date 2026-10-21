"""The fixed-work suite drivers must print exactly what the manifest expects."""
import json
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest import mock

from oxbench import suite as suite_matrix

FAKE_SUITE = """
function Benchmark(name, run, setup, tearDown) {
  this.name = name; this.run = run; this.Setup = setup || function () {}; this.TearDown = tearDown || function () {};
}
function BenchmarkSuite(name, reference, benchmarks) { this.name = name; this.benchmarks = benchmarks; BenchmarkSuite.suites.push(this); }
BenchmarkSuite.suites = [];
var calls = [];
new BenchmarkSuite('Crypto', [1, 1], [
  new Benchmark('Encrypt', function () { calls.push('e'); }, function () { calls.push('setup'); }),
  new Benchmark('Decrypt', function () { calls.push('d'); }, null, function () { calls.push('teardown'); })]);
"""


class DriverTests(unittest.TestCase):
    def node(self, source):
        node = shutil.which("node")
        if not node:
            self.skipTest("node is not installed")
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "program.js"
            path.write_text(suite_matrix.PRELUDE + source)
            return subprocess.run([node, str(path)], capture_output=True, text=True, timeout=30)

    def test_suite_driver_output_matches_expected(self):
        plan = {"Encrypt": 3, "Decrypt": 2}
        result = self.node(FAKE_SUITE + suite_matrix.suite_driver("v8-v7", "crypto", plan)
                           + "print(calls.join(' '));\n")
        self.assertEqual(result.returncode, 0, result.stderr)
        marker, calls = result.stdout.splitlines()
        self.assertEqual(marker + "\n", suite_matrix.suite_expected("v8-v7", "crypto", plan))
        self.assertEqual(calls, "setup e e e d d teardown")

    def test_unplanned_benchmark_fails(self):
        result = self.node(FAKE_SUITE + suite_matrix.suite_driver("v8-v7", "crypto", {"Encrypt": 1}))
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("unplanned benchmark Decrypt", result.stderr)

    def test_awfy_driver_checks_the_result(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for module in suite_matrix.AWFY_MODULES:
                (root / (module + ".js")).write_text("")
            (root / "bounce.js").write_text(
                "exports.newInstance = () => ({ innerBenchmarkLoop: (n) => n === 1500 });\n")
            text, expected = suite_matrix.awfy_program(root, "Bounce", 1500)
            failing, _ = suite_matrix.awfy_program(root, "Bounce", 7)
        self.assertEqual(self.node(text[len(suite_matrix.PRELUDE):]).stdout, expected)
        self.assertIn("incorrect result", self.node(failing[len(suite_matrix.PRELUDE):]).stderr)


class TableTests(unittest.TestCase):
    def test_medians_ratios_and_failures(self):
        def sample(case, engine, wall_ms, instructions, status="ok"):
            return {"case": case, "engine": engine, "status": status, "process_wall_ns": wall_ms * 1e6,
                    "hardware_counters": {"values": {"instructions": instructions}}}
        results = {"samples": [sample("a", "quickjs", 100, 1e9), sample("a", "quickjs", 120, 1e9),
                               sample("a", "oxide", 400, 4e9), sample("a", "oxide", 410, 4e9),
                               sample("b", "quickjs", 50, 5e8), sample("b", "oxide", 0, 0, "wrong-output")]}
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "results.json"
            path.write_text(json.dumps(results))
            args = SimpleNamespace(results=[path], reference="quickjs", instructions=True)
            with mock.patch("builtins.print") as printed:
                suite_matrix.table(args)
        text = printed.call_args[0][0]
        self.assertIn("| a | 110 | 1.00 | 405 | 4.00 | 3.68 | 4.0000 |", text)
        self.assertIn("| b | 50 | 0.50 | fail | – | – | – |", text)
        self.assertIn("time, geometric mean over 1 workloads: 3.68", text)
        self.assertIn("instructions, geometric mean over 1 workloads: 4.0000", text)


if __name__ == "__main__":
    unittest.main()
