"""Synthetic tests for the bounded fixed-iteration V8 runner; no JS engine."""

import copy
from pathlib import Path
import tempfile
import unittest
from unittest import mock

import iterate_v8 as runner


def source_fixture(root):
    base = root / "base.js"
    base.write_bytes(b"// original base\nMath.random = function() { return 0.5; };\n")
    run_js = root / "run.js"
    run_js.write_bytes(b"// pinned run order\n")
    paths = {"base.js": base, "run.js": run_js}
    for case, (suite, names) in runner.BENCHMARKS.items():
        declarations = ", ".join(f"new Benchmark('{name}')" for name in names)
        path = root / (case + ".js")
        path.write_bytes(f"// original {case}\r\nnew BenchmarkSuite('{suite}', 1, [{declarations}]);\r\n".encode())
        paths[case + ".js"] = path
    return {"expected_commit": "a" * 40,
            "repository": {"tree": {"stdout": "b" * 40}},
            "files": {name: {"path": str(path), "sha256": runner.digest(path)}
                      for name, path in paths.items()}}


class FixedIterationTests(unittest.TestCase):
    def test_generation_keeps_exact_bodies_and_one_setup_teardown(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = source_fixture(root)
            counts = {case: 3 for case in runner.CASES}
            generated_dir = root / "generated"
            generated_dir.mkdir()
            workload = runner.prepare_workload(source, ["crypto"], counts, 0, generated_dir, "crypto")
            generated = Path(workload["path"]).read_bytes()
            original = (root / "base.js").read_bytes() + b"\n" + (root / "crypto.js").read_bytes() + b"\n"
            self.assertTrue(generated.startswith(original))
            self.assertEqual(generated.count(b"benchmark.Setup();"), 1)
            self.assertEqual(generated.count(b"benchmark.TearDown();"), 1)
            self.assertEqual(workload["expected_stdout"],
                             f"{runner.MARKER}:crypto:1:6\n")
            self.assertNotIn(b"RunSuites(", generated)
            self.assertNotIn(b"ResetRNG", generated)

            combined = runner.prepare_workload(source, runner.CASES, counts, 0, generated_dir, "combined")
            body_order = [(root / (case + ".js")).read_bytes() for case in runner.CASES]
            combined_bytes = Path(combined["path"]).read_bytes()
            expected_prefix = b"\n".join([(root / "base.js").read_bytes(), *body_order]) + b"\n"
            self.assertTrue(combined_bytes.startswith(expected_prefix))
            self.assertEqual(combined["expected_stdout"],
                             f"{runner.MARKER}:combined:8:30\n")

    def test_calibration_respects_total_budget_and_default_matrix_size(self):
        pilot = {case: 100_000_000 for case in runner.CASES}
        pilot["combined"] = 800_000_000
        counts, estimated, feasible = runner.calibrate(pilot, 1_000_000_000, 1000,
                                                       600_000_000_000, 4)
        self.assertEqual(set(counts.values()), {10})
        self.assertEqual(estimated, 256_000_000_000)
        self.assertTrue(feasible)
        _, _, feasible = runner.calibrate(pilot, 1_000_000_000, 1000, 20_000_000_000, 4)
        self.assertFalse(feasible)
        jobs = runner.schedule([{"case": case} for case in (*runner.CASES, "combined")],
                               "abba-baab", 4)
        self.assertEqual(len(jobs), 144)
        self.assertEqual([job["engine"] for job in jobs[:8]],
                         ["same_a", "same_b", "same_b", "same_a",
                          "same_b", "same_a", "same_a", "same_b"])

    def test_replay_requires_exact_source_tooling_and_generated_bytes(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = source_fixture(root)
            counts = {case: 2 for case in runner.CASES}
            first = root / "first"
            second = root / "second"
            first.mkdir()
            second.mkdir()
            workloads = runner.workload_set(source, counts, 0, first)
            tooling = {"iterate_v8.py": {"path": "runner", "sha256": "c" * 64}}
            plan = {"schema": "oxide-v8-fixed-iteration-freeze-v1", "source": source,
                    "tooling": tooling, "order": "abba-baab",
                    "repeats_per_engine_per_phase": 4, "warmup_per_benchmark": 0,
                    "runs_per_suite": counts, "workloads": workloads}
            replayed = runner.replay_workloads(plan, source, tooling, second, "abba-baab", 4, 0)
            self.assertEqual([row["sha256"] for row in replayed],
                             [row["sha256"] for row in workloads])
            changed = copy.deepcopy(plan)
            changed["workloads"][-1]["sha256"] = "0" * 64
            with self.assertRaisesRegex(ValueError, "generated JS differs"):
                runner.replay_workloads(changed, source, tooling, second, "abba-baab", 4, 0)
            changed = copy.deepcopy(plan)
            changed["source"]["files"]["crypto.js"]["sha256"] = "0" * 64
            with self.assertRaisesRegex(ValueError, "source differs"):
                runner.replay_workloads(changed, source, tooling, second, "abba-baab", 4, 0)

    def test_incomplete_or_timeout_never_produces_aggregate(self):
        jobs = runner.schedule([{"case": case} for case in (*runner.CASES, "combined")],
                               "abba-baab", 4)
        samples = []
        for job in jobs:
            wall = 100 if job["engine"] != "candidate" else 90
            samples.append({**job, "status": "ok", "process_wall_ns": wall})
        complete = runner.summarize(samples, jobs, 4)
        self.assertTrue(complete["complete"])
        self.assertAlmostEqual(complete["aggregate"]["combined_baseline_over_candidate_wall"],
                               100 / 90)
        self.assertEqual(complete["cases"]["crypto"]["interpretation"],
                         "outside_observed_aa_span_not_admission")
        self.assertIsNone(runner.summarize(samples[:-1], jobs, 4)["aggregate"])
        timed_out = copy.deepcopy(samples)
        timed_out[-1]["status"] = "timeout"
        self.assertIsNone(runner.summarize(timed_out, jobs, 4)["aggregate"])
        noisy = copy.deepcopy(samples)
        for row in noisy:
            if row["engine"] == "same_b":
                row["process_wall_ns"] = 130
        self.assertEqual(runner.summarize(noisy, jobs, 4)["cases"]["crypto"]["interpretation"],
                         "inconclusive_within_aa_span")

    def test_expired_deadline_does_not_start_process(self):
        with mock.patch.object(runner.time, "monotonic", return_value=100), \
             mock.patch.object(runner, "run_sample") as launched:
            sample = runner.run_one({}, {}, {}, {}, Path("/tmp"), Path("/tmp/sample"),
                                    99, 10, False, "pilot", 0, "baseline")
        self.assertIsNone(sample)
        launched.assert_not_called()


if __name__ == "__main__":
    unittest.main()
