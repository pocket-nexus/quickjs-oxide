"""Fast, synthetic tests of result admission, timeout handling and aggregation."""
import importlib.util
from pathlib import Path
import sys
import tempfile
import unittest
from unittest import mock

spec = importlib.util.spec_from_file_location("benchmark_run", Path(__file__).with_name("run.py"))
runner = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runner)


class Results(unittest.TestCase):
    def test_swallowed_failure_is_not_a_score(self):
        for output in ["Richards: Error: failed\n", "Score: 123\n", "Richards: 123\nScore: 0\n", "Richards: 2\nRichards: 3\nScore: 4\n"]:
            with self.assertRaises(ValueError):
                runner.parse_v8(output, ["Richards"])
        self.assertEqual(runner.parse_v8("Richards: 123\n----\nScore: 123\n", ["Richards"])["Score"], 123)

    def test_microbench_requires_every_selected_result(self):
        with self.assertRaises(ValueError):
            runner.parse_microbench(" prop_read 1000 2.3\n", ["prop_read", "prop_write"])
        self.assertEqual(runner.parse_microbench("__oxide_clock__:Date.now\n prop_read 1000 2.3\n total 2.3\n", ["prop_read"])["prop_read"]["ns_per_op"], 2.3)

    def test_microbench_rejects_wrong_clock_even_with_valid_results(self):
        with self.assertRaises(ValueError):
            runner.parse_microbench("__oxide_clock__:performance.now\n prop_read 1000 2.3\n", ["prop_read"])

    def test_clock_adaptation_preserves_every_original_body_byte(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "microbench.js"
            original = b"// synthetic fixture\r\nvar test_list = [empty_loop];\r\nfunction empty_loop(n) { return n; }\r\n"
            source.write_bytes(original)
            workloads, metadata = runner.prepare_microbench(source, ["empty_loop"])
            prepared = Path(workloads[0]["path"])
            self.assertEqual(prepared.read_bytes(), runner.MICROBENCH_CLOCK_PREFIX.encode() + original)
            self.assertEqual(source.read_bytes(), original)
            self.assertNotEqual(metadata["sha256"], metadata["prepared_sha256"])
            prepared.unlink()
            prepared.parent.rmdir()

    def test_one_failed_repetition_disqualifies_comparison(self):
        summary = runner.summarize([
            {"case": "x", "engine": "a", "status": "ok", "measurements": {"Score": 12}},
            {"case": "x", "engine": "a", "status": "timeout"},
        ], "v8-v7")[0]
        self.assertFalse(summary["eligible_for_comparison"])
        self.assertEqual(summary["successful"], 1)
        self.assertEqual(summary["metrics"]["Score"]["raw"], [12])

    def test_timeout_is_retained_with_raw_output(self):
        with tempfile.TemporaryDirectory() as directory:
            prefix = Path(directory) / "sample"
            result = runner.run_sample([sys.executable, "-c", "import time; print('started', flush=True); time.sleep(2)"], directory, prefix, .2)
            self.assertTrue(result["timed_out"])
            self.assertNotEqual(result["exit_code"], 0)
            self.assertIn("started", prefix.with_suffix(".stdout").read_text())

    def test_changed_inputs_abort_before_next_sample(self):
        for changed in ("workload", "engine"):
            with self.subTest(changed=changed), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                workload_path = root / "workload.js"
                workload_path.write_text("// frozen workload\n")
                engine_path = root / "engine"
                engine_path.write_text("frozen engine\n")
                engine_path.chmod(0o755)
                workload = {"case": "empty_loop", "path": str(workload_path), "args": [],
                            "sha256": runner.digest(workload_path), "expected": ["empty_loop"]}
                engine_sha256 = runner.digest(engine_path)
                calls = []

                def fake_sample(command, cwd, prefix, timeout):
                    calls.append(command)
                    stdout = prefix.with_suffix(".stdout")
                    stderr = prefix.with_suffix(".stderr")
                    stdout.write_text("__oxide_clock__:Date.now\nempty_loop 1 1\n")
                    stderr.write_text("")
                    path = workload_path if changed == "workload" else engine_path
                    path.write_bytes(path.read_bytes() + b"changed\n")
                    return {"command": command, "exit_code": 0, "timed_out": False,
                            "process_wall_ns": 1, "stdout": str(stdout), "stderr": str(stderr)}

                argv = ["run.py", "--suite", "microbench", "--source", str(workload_path),
                        "--engine", f"candidate={engine_path}", "--repeat", "2",
                        "--output", str(root / "results")]
                with mock.patch.object(sys, "argv", argv), \
                     mock.patch.object(runner, "prepare_microbench", return_value=([workload], {})), \
                     mock.patch.object(runner, "machine_metadata", return_value={}), \
                     mock.patch.object(runner, "binary_metadata", return_value={"sha256": engine_sha256}), \
                     mock.patch.object(runner, "run_sample", side_effect=fake_sample):
                    with self.assertRaisesRegex(ValueError, f"{changed} changed during measurement"):
                        runner.main()
                self.assertEqual(len(calls), 1)


if __name__ == "__main__":
    unittest.main()
