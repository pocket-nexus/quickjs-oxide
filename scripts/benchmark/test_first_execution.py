"""First-execution receipts keep semantic failures out of timing comparisons."""
import argparse
import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from first_execution import (PROBE_SCHEMA, PROBE_SOURCE, admit, configuration_signature, load_workloads,
                             prepare, probe_identity, schedule)
from replay import summarize
from run import digest


class FirstExecutionTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.stdout, self.stderr = self.root / "out", self.root / "err"
        self.stdout.write_bytes(b"42\n")
        self.stderr.write_bytes(b"")
        self.metrics = self.root / "metrics.json"
        self.metric = dict(schema=PROBE_SCHEMA, compile_ns=200, first_execute_ns=42,
                           compile_count=1, execute_count=1, pending_jobs=0, profiling=False)
        self.metrics.write_text(json.dumps(self.metric))
        self.sample = dict(timed_out=False, exit_code=0, stdout=str(self.stdout), stderr=str(self.stderr))
        self.workload = dict(case="answer", expected="42\n")

    def binary(self):
        binary = self.root / "probe"
        binary.write_text("never execute this fixture\n")
        binary.chmod(0o700)
        build = dict(command=["cargo", "build", "--release"], source_sha256=digest(PROBE_SOURCE),
                     source_identity={"kind": "test-fixture"}, rustc="test compiler\nhost: x86_64-unknown-linux-gnu\n",
                     rustflags=None, encoded_rustflags=None, features=[],
                     cargo_lock_sha256="0" * 64, binary_sha256=digest(binary))
        build["build_provenance"] = dict(schema="oxide-probe-build-provenance.v1",
                                        environment={"CARGO_PROFILE_RELEASE_LTO": "fat", "CARGO_PROFILE_RELEASE_CODEGEN_UNITS": "1"},
                                        cargo_configurations=[], probe_release_profile={})
        binary.with_suffix(".build.json").write_text(json.dumps(build))
        return binary

    def test_only_direct_execute_time_is_admitted(self):
        status, metrics, diagnostic = admit(self.sample, self.workload, self.metrics)
        self.assertEqual((status, metrics), ("ok", {"first_execute_ns": 42}))
        self.assertEqual(diagnostic["compile_ns"], 200)
        self.assertNotIn("process_wall_ns", metrics)

    def test_failures_and_async_instrumented_or_repeated_execution_are_not_zero(self):
        variants = [({"pending_jobs": 1}, "unsupported-pending-jobs"),
                    ({"profiling": True}, "instrumented-probe"),
                    ({"execute_count": 2}, "invalid-metrics"),
                    ({"compile_count": 2}, "invalid-metrics"),
                    ({"first_execute_ns": 0}, "invalid-metrics"),
                    ({"first_execute_ns": True}, "invalid-metrics"),
                    ({"compile_ns": -1}, "invalid-metrics")]
        for changed, expected in variants:
            with self.subTest(changed=changed):
                self.metrics.write_text(json.dumps({**self.metric, **changed}))
                status, metrics, _ = admit(self.sample, self.workload, self.metrics)
                self.assertEqual(status, expected)
                self.assertEqual(metrics, {})
        self.metrics.unlink()
        self.assertEqual(admit(self.sample, self.workload, self.metrics)[0], "missing-or-malformed-metrics")
        self.assertEqual(admit({**self.sample, "timed_out": True}, self.workload, self.metrics)[0], "timeout")
        self.assertEqual(admit({**self.sample, "exit_code": 1}, self.workload, self.metrics)[0], "failed")
        self.stdout.write_bytes(b"42\nextra\n")
        self.assertEqual(admit(self.sample, self.workload, self.metrics)[0], "invalid-stdout")
        self.stderr.write_bytes(b"warning\n")
        self.assertEqual(admit(self.sample, self.workload, self.metrics)[0], "unexpected-stderr")

    def test_bad_sample_invalidates_ratio_but_keeps_successful_raw_measurements(self):
        samples = [dict(case="answer", engine=name, status="ok", measurements={"first_execute_ns": value})
                   for name, value in [("M0", 40), ("M0", 42), ("M1", 43), ("M1", 44)]]
        samples[-1].update(status="failed", measurements={})
        rows, ratios = summarize(samples, ["M0", "M1"])
        self.assertEqual(ratios, [])
        self.assertEqual(rows[-1]["metrics"]["first_execute_ns"]["raw"], [43])

    def test_three_modes_rotate_and_keep_ten_independent_samples_each(self):
        entries = schedule([self.workload], ["M0", "M1", "M2"])
        self.assertEqual([item["engine"] for item in entries[:9]],
                         ["M0", "M1", "M2", "M1", "M2", "M0", "M2", "M0", "M1"])
        for mode in ("M0", "M1", "M2"):
            self.assertEqual(sum(item["engine"] == mode for item in entries), 10)

    def test_prepare_freezes_receipts_and_never_launches_candidate(self):
        binary = self.binary()
        source = self.root / "answer.js"
        source.write_text("print(42);\n")
        manifest = self.root / "fixed.json"
        workload = dict(self.workload, path=str(source), sha256=digest(source))
        manifest.write_text(json.dumps({"metadata": {"workloads": {"workloads": [workload]}}}))
        output = self.root / "prepared"
        args = argparse.Namespace(timeout=180, engine=[f"M1={binary}", f"M0={binary}"],
                                  manifest=manifest, workload_dir=None, case=None, output=output)
        with patch("first_execution.os.sched_getaffinity", return_value={2}), \
             patch("first_execution.shutil.which", return_value=str(binary)), \
             patch("first_execution.machine_metadata", return_value={}), \
             patch("first_execution.run_sample") as launch:
            self.assertEqual(prepare(args), 0)
        launch.assert_not_called()
        protocol = json.loads((output / "protocol.json").read_text())
        self.assertEqual((protocol["cpu"], protocol["repeat"], len(protocol["schedule"])), (2, 10, 20))
        self.assertEqual(list(protocol["engines"]), ["M0", "M1"])
        self.assertEqual((output / "protocol.sha256").read_text().strip(), digest(output / "protocol.json"))
        source.write_text("print(41);\n")
        with self.assertRaisesRegex(ValueError, "bytes changed"):
            load_workloads(manifest)

    def test_stale_or_instrumented_build_receipt_is_rejected_without_execution(self):
        binary = self.binary()
        probe_identity(binary)
        receipt = binary.with_suffix(".build.json")
        build = json.loads(receipt.read_text())
        build["features"] = ["profiling"]
        receipt.write_text(json.dumps(build))
        with self.assertRaisesRegex(ValueError, "instrumentation"):
            probe_identity(binary)
        build["features"] = []
        receipt.write_text(json.dumps(build))
        binary.write_text("changed fixture\n")
        with self.assertRaisesRegex(ValueError, "stale"):
            probe_identity(binary)

    def test_only_approved_cfgs_may_differ_and_pgo_is_rejected(self):
        binary = self.binary()
        build = json.loads(binary.with_suffix(".build.json").read_text())
        baseline = configuration_signature(build)
        for flags in ("--cfg oxide_quick_projection", "--cfg=oxide_quick_dispatch --cfg oxide_quick_projection"):
            build["rustflags"] = flags
            build["build_provenance"]["environment"]["RUSTFLAGS"] = flags
            self.assertEqual(configuration_signature(build), baseline)
        for flags in ("-Cprofile-use=/tmp/data", "-C profile-generate=/tmp/data", '--cfg feature="profiling"'):
            build["rustflags"] = flags
            build["build_provenance"]["environment"]["RUSTFLAGS"] = flags
            with self.assertRaisesRegex(ValueError, "PGO|profiling"):
                configuration_signature(build)
        build["rustflags"] = "--cfg oxide_scalar_tos"
        build["build_provenance"]["environment"]["RUSTFLAGS"] = build["rustflags"]
        self.assertNotEqual(configuration_signature(build), baseline)
        build["build_provenance"]["environment"]["CARGO_PROFILE_RELEASE_LTO"] = "thin"
        with self.assertRaisesRegex(ValueError, "fat LTO"):
            configuration_signature(build)

    def test_target_and_wrapper_changes_affect_configuration_signature(self):
        binary = self.binary()
        build = json.loads(binary.with_suffix(".build.json").read_text())
        baseline = configuration_signature(build)
        build["build_provenance"]["environment"]["CARGO_BUILD_TARGET"] = "aarch64-unknown-linux-gnu"
        self.assertNotEqual(configuration_signature(build), baseline)
        del build["build_provenance"]["environment"]["CARGO_BUILD_TARGET"]
        build["build_provenance"]["environment"]["RUSTC_WRAPPER"] = "/wrapper"
        with self.assertRaisesRegex(ValueError, "wrapper identity"):
            configuration_signature(build)
        build["build_provenance"]["compiler_tools"] = {"RUSTC_WRAPPER": dict(path="/wrapper", sha256="1" * 64)}
        self.assertNotEqual(configuration_signature(build), baseline)


if __name__ == "__main__":
    unittest.main()
