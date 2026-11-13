"""Layout pressure rejects partial evidence and distinguishes spread from work."""
from pathlib import Path
import tempfile
import unittest
from unittest import mock

from oxbench import layout, relink
from oxbench.common import paired_order


def results(base=100, candidate=100):
    engines = {"base": {}, "candidate": {}}
    return dict(metadata=dict(engines=engines, repeat=4, order_strategy="abba", cpu=2,
                              linux_counters=["instructions:u", "cycles:u", "task-clock"],
                              workloads=[dict(case="crypto", sha256="a" * 64)]),
                samples=[dict(case="crypto", engine=e, repetition=r, status="ok", exit_code=0,
                              timed_out=False, semantic_status="ok", process_wall_ns=value,
                              hardware_counters=dict(values=dict(instructions=1000, cycles=value * 4)))
                         for r in range(4) for e in paired_order(list(engines), r, "abba")
                         for value in [base if e == "base" else candidate]])


class LayoutTests(unittest.TestCase):
    def test_unchanged_instructions_do_not_hide_time_and_cycles_regression(self):
        report = layout.summarize({d: results(candidate=112) for d in layout.OFFSETS}, 3, 2)
        self.assertFalse(report["gate_passed"])
        self.assertEqual(report["rows"][0]["ratios"]["instructions"], [1] * 4)
        self.assertEqual(report["rows"][0]["failures"], ["time regression", "cycles regression"])

    def test_good_median_does_not_hide_bad_placement(self):
        report = layout.summarize({d: results(candidate=110 if d == 48 else 99) for d in layout.OFFSETS}, 3, 2)
        row = report["rows"][0]
        self.assertEqual(row["engines"]["candidate"]["time"]["median"], 99)
        self.assertEqual(row["engines"]["candidate"]["time"]["max"], 110)
        self.assertIn("time spread", row["failures"])
        self.assertIn("time regression", row["failures"])
        self.assertIn("FAIL", layout.markdown(report))

    def test_stable_improvement_passes(self):
        report = layout.summarize({d: results(candidate=98) for d in layout.OFFSETS}, 3, 2)
        self.assertTrue(report["gate_passed"])
        self.assertIn("Gate: pass", layout.markdown(report))

    def test_failed_missing_reordered_or_counterless_samples_are_rejected(self):
        for mutation in [lambda s: s.pop(), lambda s: s.reverse(),
                         lambda s: s[0].update(status="failed"),
                         lambda s: s[0]["hardware_counters"]["values"].pop("cycles")]:
            data = results()
            mutation(data["samples"])
            with self.assertRaises(ValueError):
                layout.matrix_medians(data)

    def test_all_offsets_and_identical_inputs_are_required(self):
        data = {d: results() for d in layout.OFFSETS}
        data[48]["metadata"]["workloads"][0]["sha256"] = "b" * 64
        with self.assertRaisesRegex(ValueError, "inputs"):
            layout.summarize(data, 3, 2)
        data.pop(48)
        with self.assertRaisesRegex(ValueError, "offsets"):
            layout.summarize(data, 3, 2)

    def test_cpu_or_machine_changes_invalidate_the_comparison(self):
        for key, value in (("cpu", 3), ("machine", {"cpu": "another CPU"})):
            data = {d: results() for d in layout.OFFSETS}
            data[48]["metadata"][key] = value
            with self.assertRaisesRegex(ValueError, "settings"):
                layout.summarize(data, 3, 2)

    def test_strict_relinks_reject_changed_text(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            log = root / "build.log"
            log.write_text('"cc" "input.o" "-o" "/tmp/qjs-hash"\n')
            with mock.patch.object(relink.subprocess, "run"), \
                    mock.patch.object(relink, "text_info", side_effect=[(0x1000, 10, "default"),
                                                                       (0x1000, 10, "same"),
                                                                       (0x1010, 10, "changed")]):
                with self.assertRaisesRegex(ValueError, "text differs"):
                    relink.placements(log, root / "qjs", [0, 16], binary="qjs",
                                      require_identical=True)


if __name__ == "__main__":
    unittest.main()
