"""The sweep reads the gates from its own two runs and the candidate's symbols."""
import contextlib
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest import mock

from oxbench import sweep


def results(cases, ratio):
    samples = []
    for case in cases:
        for engine, instructions in (("base", 1000), ("candidate", 1000 * ratio.get(case, 1.0))):
            samples.append({"case": case, "engine": engine, "status": "ok", "process_wall_ns": 3e8,
                            "hardware_counters": {"values": {"instructions": instructions}}})
    return {"samples": samples}


class SweepTests(unittest.TestCase):
    def test_loop_size_reads_the_run_at_symbol(self):
        listing = ("00000000000ce9b0 0000000000003b6a t oxide_core::vm::interp::run_at\n"
                   "00000000000d0000 0000000000000010 t oxide_core::vm::interp::run_at_slow\n")
        with mock.patch.object(sweep.subprocess, "run", return_value=mock.Mock(stdout=listing)):
            self.assertEqual(sweep.loop_size("qjs"), 0x3b6a)
        with mock.patch.object(sweep.subprocess, "run", return_value=mock.Mock(stdout="")), \
                self.assertRaisesRegex(ValueError, "no vm::interp::run_at"):
            sweep.loop_size("qjs")

    def sweep(self, v8_ratio, fixed_ratio=None, size=0x3b6a, status=0):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            manifest = root / "manifest.json"
            manifest.write_text("{}")

            def run(argv):
                output = Path(argv[argv.index("--output") + 1])
                output.mkdir()
                fixed = "--workload-dir" in argv
                cases = ["empty_loop"] if fixed else [f"v8-v7-{case}" for case in sweep.V8_CASES]
                (output / "results.json").write_text(json.dumps(results(cases, (fixed_ratio or {}) if fixed else v8_ratio)))
                return status

            with mock.patch.object(sweep.runner, "main", side_effect=run), \
                    mock.patch.object(sweep, "loop_size", return_value=size), \
                    mock.patch.object(sweep.platform, "system", return_value="Linux"), \
                    contextlib.redirect_stdout(io.StringIO()):
                code = sweep.main(["--base", "base", "--candidate", "candidate", "--suite", str(manifest),
                                   "--output", str(root / "out")])
            return code, (root / "out" / "summary.md").read_text()

    def test_passing_sweep(self):
        code, summary = self.sweep({"v8-v7-richards": 1.019})
        self.assertEqual(code, 0)
        self.assertIn("Instruction gate (eight V8 v7 programs, at most ×1.02): pass", summary)
        self.assertIn("15210 bytes, pass", summary)

    def test_instruction_and_loop_size_gates_fail(self):
        code, summary = self.sweep({"v8-v7-splay": 1.021})
        self.assertEqual(code, 1)
        self.assertIn("v8-v7-splay: instructions ×1.0210", summary)
        code, summary = self.sweep({}, size=16_384)
        self.assertEqual(code, 1)
        self.assertIn("16384 bytes, FAIL", summary)

    def test_fixed_probe_above_bound_is_flagged_not_failed(self):
        code, summary = self.sweep({}, fixed_ratio={"empty_loop": 1.05})
        self.assertEqual(code, 0)
        self.assertIn("empty_loop: instructions ×1.0500", summary)

    def test_failed_sample_fails_the_sweep(self):
        code, summary = self.sweep({}, status=1)
        self.assertEqual(code, 1)
        self.assertIn("some failed", summary)


if __name__ == "__main__":
    unittest.main()
