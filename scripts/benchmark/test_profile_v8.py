"""Small admission tests for the fixed V8 diagnostic profiler."""
import json
from pathlib import Path
import tempfile
import unittest
from unittest import mock

import profile_v8 as profiler


class FixedV8Profile(unittest.TestCase):
    def test_upstream_declarations_are_checked_before_generation(self):
        source = "new BenchmarkSuite('Crypto', [], [new Benchmark('Encrypt'), new Benchmark('Decrypt')]);"
        self.assertEqual(profiler.declared_benchmarks(source, "crypto"),
                         ("Crypto", ("Encrypt", "Decrypt")))
        with self.assertRaisesRegex(ValueError, "declarations differ"):
            profiler.declared_benchmarks(source.replace("Decrypt", "DecryptWrong"), "crypto")

    def test_generated_workload_keeps_body_and_uses_fixed_driver(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            base = root / "base.js"
            body = root / "crypto.js"
            base.write_bytes(b"var BenchmarkSuite = {suites: []};\n")
            original = b"new BenchmarkSuite('Crypto', [], [new Benchmark('Encrypt'), new Benchmark('Decrypt')]);\n"
            body.write_bytes(original)
            generated = root / "generated"
            generated.mkdir()
            files = {name: {"path": str(path), "sha256": profiler.digest(path)}
                     for name, path in (("base.js", base), ("crypto.js", body))}
            item = profiler.prepare_case({"files": files}, "crypto", 2, generated)
            program = Path(item["path"]).read_bytes()
            self.assertTrue(program.startswith(base.read_bytes() + b"\n" + original + b"\n"))
            self.assertIn(b"benchmark.Setup();\n      benchmark.run();\n      benchmark.TearDown();", program)
            self.assertNotIn(b"RunSuites(", program)
            self.assertEqual(item["expected_marker"],
                             "__oxide_v8_fixed_profile_complete__:Crypto:2:2:4\n")
            self.assertEqual(item["source_sha256"], profiler.digest(body))
            self.assertEqual(item["sha256"], profiler.digest(item["path"]))

    def test_pinned_checkout_and_dirty_tracking_are_required(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory)
            repository = {"commit": {"exit_code": 0, "stdout": "0" * 40},
                          "tree": {"exit_code": 0, "stdout": "1" * 40},
                          "top_level": {"exit_code": 0, "stdout": str(source)}}
            with mock.patch.object(profiler, "git_metadata", return_value=repository):
                with self.assertRaisesRegex(ValueError, "pinned"):
                    profiler.validate_source(source, profiler.V8_V7_SOURCE_COMMIT)
            repository["commit"]["stdout"] = profiler.V8_V7_SOURCE_COMMIT
            with mock.patch.object(profiler, "git_metadata", return_value=repository), \
                 mock.patch.object(profiler, "command_output", return_value={"exit_code": 0, "stdout": " M v8-v7/run.js"}):
                with self.assertRaisesRegex(ValueError, "tracked source"):
                    profiler.validate_source(source, profiler.V8_V7_SOURCE_COMMIT)

    def test_profile_json_requires_cost_and_records_omissions(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "profile.jsonl"
            record = {"schema": profiler.COST_SCHEMA,
                      "metadata": {"profiling_feature": True, "commit": "a" * 40},
                      "fusion_diagnostics": {
                          "callsite_scope": "ordinary-driver-enter-selected-only",
                          "omitted": {"static_functions": 1, "dispatch_events": 2,
                                      "outcome_events": 3, "callsite_events": 4},
                          "functions": [{"bytecode_id": 1}], "dispatch": [],
                          "sites": [], "callsites": []},
                      "vm_phases": {"run": {"omitted_samples": 5}},
                      "unavailable": ["compile-peak-memory"]}
            path.write_text(json.dumps(record) + "\n")
            summary = profiler.parse_cost_json(path, "a" * 40)
            self.assertEqual(summary["fusion_omitted"]["callsite_events"], 4)
            self.assertEqual(summary["vm_phase_omitted_samples"], {"run": 5})
            self.assertEqual(summary["fusion_counts"]["functions"], 1)
            with self.assertRaisesRegex(ValueError, "embedded commit"):
                profiler.parse_cost_json(path, "b" * 40)
            record["fusion_diagnostics"]["omitted"].pop("callsite_events")
            path.write_text(json.dumps(record) + "\n")
            with self.assertRaisesRegex(ValueError, "omission counters"):
                profiler.parse_cost_json(path)


if __name__ == "__main__":
    unittest.main()
