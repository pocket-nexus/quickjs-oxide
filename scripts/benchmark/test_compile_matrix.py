"""Front-end matrix admits only exact probe output and never hides failed rounds."""
import json
import tempfile
import unittest
from pathlib import Path

from compile_matrix import admit, classify, engine_command, load_corpus, summarize
from compile_workloads import generate
from run import digest


class CompileMatrixTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)

    def sample(self, stdout, stderr="", exit_code=0, timed_out=False):
        stdout_path, stderr_path = self.root / "stdout", self.root / "stderr"
        stdout_path.write_text(stdout)
        stderr_path.write_text(stderr)
        return dict(timed_out=timed_out, exit_code=exit_code, stdout=str(stdout_path), stderr=str(stderr_path))

    def test_probe_versions_classify_or_fail(self):
        self.assertEqual(classify("oxide-compile-probe 1\n"), "oxide")
        self.assertEqual(classify("quickjs-compile-probe 1\n"), "quickjs")
        self.assertEqual(classify("boa-compile-probe 1\n"), "boa")
        self.assertEqual(classify("v24.21.0\n"), "node")
        with self.assertRaises(ValueError):
            classify("qjs 2026-06-04\n")
        with self.assertRaises(ValueError):
            classify("oxide-compile-alloc-probe 1\n")

    def test_node_commands_use_eager_and_parse_only_flags(self):
        compile_command = engine_command("node", Path("/usr/bin/node"), "compile", Path("/tmp/a.js"))
        self.assertIn("--no-lazy", compile_command)
        parse_command = engine_command("node", Path("/usr/bin/node"), "parse", Path("/tmp/a.js"))
        self.assertIn("--parse-only", parse_command)
        self.assertEqual(engine_command("boa", Path("/tmp/boa"), "parse", Path("/tmp/a.js"))[1], "parse")

    def test_output_requires_exact_framing_and_empty_stderr(self):
        self.assertEqual(admit(self.sample("compile_ns:42\n"), "compile"), ("ok", {"compile_ns": 42}))
        self.assertEqual(admit(self.sample("compile_ns:42\nextra\n"), "compile")[0], "invalid-output")
        self.assertEqual(admit(self.sample("compile_ns:0\n"), "compile")[0], "invalid-output")
        self.assertEqual(admit(self.sample("parse_ns:42\n"), "compile")[0], "invalid-output")
        self.assertEqual(admit(self.sample("compile_ns:42\n", stderr="warning\n"), "compile")[0], "unexpected-stderr")
        self.assertEqual(admit(self.sample("compile_ns:42\n", exit_code=1), "compile")[0], "failed")
        self.assertEqual(admit(self.sample("", timed_out=True), "compile")[0], "timeout")

    def test_corpus_manifest_authenticates_generated_sources(self):
        directory = self.root / "corpus"
        directory.mkdir()
        path = directory / "functions-64.js"
        path.write_text(generate("functions", 64))
        manifest = {"schema": "oxide-compile-corpus-v1",
                    "workloads": [{"case": "functions-64", "path": str(path), "bytes": path.stat().st_size,
                                   "sha256": digest(path)}]}
        (directory / "manifest.json").write_text(json.dumps(manifest))
        self.assertEqual(len(load_corpus(directory, ["functions-64"])), 1)
        path.write_text("// changed\n")
        with self.assertRaisesRegex(ValueError, "changed since manifest"):
            load_corpus(directory, None)
        with self.assertRaises(ValueError):
            load_corpus(directory, ["missing"])

    def test_failed_round_disqualifies_the_case_ratio(self):
        workloads = [{"case": "sample", "bytes": 1000}]
        samples = [dict(case="sample", engine=name, status="ok", measurements={"compile_ns": value})
                   for name, value in [("reference", 100), ("reference", 120), ("candidate", 50), ("candidate", 60)]]
        results, ratios = summarize(samples, ["reference", "candidate"], workloads)
        self.assertEqual(ratios[0]["ratio"], 110 / 55)
        samples[-1].update(status="failed", measurements={})
        results, ratios = summarize(samples, ["reference", "candidate"], workloads)
        self.assertEqual(ratios, [])
        candidate = next(group for group in results if group["engine"] == "candidate")
        self.assertFalse(candidate["eligible"])
        self.assertEqual(candidate["metrics"]["ns"]["raw"], [50])


if __name__ == "__main__":
    unittest.main()
