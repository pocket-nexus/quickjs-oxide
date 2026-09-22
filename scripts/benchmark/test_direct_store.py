"""C1 workload contracts and fixed-runner admission, independent of engine timings."""
import json
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

import direct_store
from fixed import load_workloads


class DirectStoreWorkloads(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.output = self.root / "capture"

    def test_generated_manifest_is_admitted_by_existing_fixed_runner(self):
        manifest = direct_store.prepare(self.output, 17)
        rows = load_workloads(manifest)
        self.assertEqual([row["case"] for row in rows], list(direct_store.CASES))
        self.assertTrue(all(row["operations"] == row["size"] == 17 for row in rows))
        self.assertTrue(all(row["description"] for row in rows))
        self.assertEqual(len(json.loads(manifest.read_text())["metadata"]["generator_sha256"]), 64)
        relocated = self.root / "relocated"
        shutil.copytree(self.output / "workloads", relocated)
        relocated_rows = load_workloads(manifest, relocated)
        self.assertEqual([row["sha256"] for row in rows], [row["sha256"] for row in relocated_rows])
        self.assertEqual([row["expected"] for row in rows], [row["expected"] for row in relocated_rows])

    def test_modified_workload_is_rejected_by_existing_runner(self):
        manifest = direct_store.prepare(self.output, 2, ["arg-keep-scalar"])
        path = self.output / "workloads/arg-keep-scalar.js"
        path.write_text(path.read_text() + "// changed\n")
        with self.assertRaisesRegex(ValueError, "bytes changed"):
            load_workloads(manifest)

    def test_generation_does_not_overwrite_existing_capture(self):
        manifest = direct_store.prepare(self.output, 2)
        before = manifest.read_bytes()
        with self.assertRaises(FileExistsError):
            direct_store.prepare(self.output, 3)
        self.assertEqual(manifest.read_bytes(), before)

    def test_invalid_selection_or_iterations_leaves_no_capture(self):
        for iterations, cases in [(0, None), (-1, None), (True, None),
                                  (direct_store.MAX_ITERATIONS + 1, None),
                                  (1, []), (1, ["unknown"]),
                                  (1, ["arg-keep-scalar", "arg-keep-scalar"])]:
            with self.subTest(iterations=iterations, cases=cases), self.assertRaises(ValueError):
                direct_store.prepare(self.output, iterations, cases)
            self.assertFalse(self.output.exists())

    def test_large_iteration_contracts_are_bounded_and_exact(self):
        for case in direct_store.CASES:
            with self.subTest(case=case):
                _, row = direct_store.workload(case, direct_store.MAX_ITERATIONS)
                for value in json.loads(row["expected"]):
                    if isinstance(value, int):
                        self.assertLess(abs(value), 1 << 53)

    @unittest.skipUnless(shutil.which("node"), "Node is required for independent workload smoke checks")
    def test_every_workload_matches_independent_expected_stdout(self):
        # Cross both the heap alternation boundary and scalar 256-value period.
        # Consume cases also verify loop var hoisting and parameter var
        # redeclaration semantics; only engine profiling proves PutLocal/PutArg.
        for iterations in [1, 2, 17, 257]:
            manifest = direct_store.prepare(self.root / str(iterations), iterations)
            for row in load_workloads(manifest):
                with self.subTest(case=row["case"], iterations=iterations):
                    result = subprocess.run(["node", row["path"]], capture_output=True,
                                            text=True, timeout=10)
                    self.assertEqual(result.returncode, 0, result.stderr)
                    self.assertEqual(result.stderr, "")
                    self.assertEqual(result.stdout, row["expected"])


if __name__ == "__main__":
    unittest.main()
