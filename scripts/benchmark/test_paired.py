"""Paired inference must preserve signs, correspondence and admission boundaries."""
import json
from pathlib import Path
import random
import statistics
import sys
import tempfile
import unittest
from unittest import mock

import paired
from run import digest, paired_order


def fixture(metric="fixed-time", cases=None, gains=None, repeat=6, order="abba", same_binary=False):
    cases = cases or ["richards"]
    gains = gains or [0.1] * repeat
    engines = {"base": {"sha256": "a" * 64},
               "new": {"sha256": ("a" if same_binary else "b") * 64}}
    workloads = [dict(case=case, sha256=f"{index:064x}",
                      expected=list(paired.V8_SCORE_NAMES.values()) if case == "all"
                      else [paired.V8_SCORE_NAMES.get(case, case.title())])
                 for index, case in enumerate(cases, start=1)]
    metadata = dict(engines=engines, repeat=repeat, order_strategy=order,
                    workloads=workloads, cpu=2,
                    machine=dict(cpu="frozen-cpu", cpu_affinity=[2]))
    if metric == "fixed-time":
        metadata["metric"] = "whole-process wall nanoseconds; not adaptive scores"
    else:
        metadata["suite"] = "v8-v7"
    samples = []
    for workload in workloads:
        for repetition in range(repeat):
            base = 100 * (repetition + 1)
            new = base * (1 - gains[repetition] if metric == "fixed-time" else 1 + gains[repetition])
            for engine in paired_order(list(engines), repetition, order):
                sample = dict(case=workload["case"], engine=engine, repetition=repetition,
                              exit_code=0, timed_out=False, status="ok")
                value = base if engine == "base" else new
                if metric == "fixed-time":
                    sample["process_wall_ns"] = value
                else:
                    sample["measurements"] = {name: value for name in workload["expected"] + ["Score"]}
                samples.append(sample)
    return dict(metadata=metadata, samples=samples) if metric == "fixed-time" else dict(**metadata, samples=samples)


class PairedStatisticsTests(unittest.TestCase):
    def test_gain_sign_and_dimension_are_explicit(self):
        base = [100] * 6
        for metric, new, expected in [("fixed-time", 80, 20), ("original-score", 120, 20),
                                      ("fixed-time", 120, -20), ("original-score", 80, -20)]:
            with self.subTest(metric=metric, new=new):
                result = paired.paired_statistics(base, [new] * 6, metric)
                self.assertAlmostEqual(result["median_gain_percent"], expected)
                for bound in result["gain_ci95_percent"]:
                    self.assertAlmostEqual(bound, expected)

    def test_estimator_uses_corresponding_ratios_not_ratio_of_medians(self):
        base = [1, 2, 3, 100, 200, 300]
        new = [3, 6, 9, 100, 200, 300]
        result = paired.paired_statistics(base, new, "original-score")
        self.assertEqual(result["median_gain_percent"], 100)
        self.assertNotEqual(result["median_gain_percent"], 100 * (statistics.median(new) / statistics.median(base) - 1))

    def test_bootstrap_matches_independent_paired_resampling(self):
        base = [100] * 6
        new = [80, 84, 91, 95, 98, 99]
        gains = [100 * (1 - value / 100) for value in new]
        rng = random.Random(79)
        estimates = []
        for _ in range(10000):
            draw = [gains[int(rng.random() * len(gains))] for _ in gains]
            estimates.append(statistics.median(draw))
        estimates.sort()
        result = paired.paired_statistics(base, new, "fixed-time")
        self.assertEqual(result["gain_ci95_percent"], [estimates[249], estimates[9749]])
        self.assertEqual(result, paired.paired_statistics(base, new, "fixed-time"))

    def test_bad_observations_and_pair_count_are_rejected(self):
        for value in [0, -1, float("nan"), float("inf"), True, "100"]:
            with self.subTest(value=value), self.assertRaisesRegex(ValueError, "positive and finite"):
                paired.paired_statistics([value] * 6, [100] * 6, "fixed-time")
        for base, new in [([1] * 5, [1] * 5), ([1] * 6, [1] * 7)]:
            with self.assertRaisesRegex(ValueError, "complete pairs"):
                paired.paired_statistics(base, new, "fixed-time")


class AdmissionTests(unittest.TestCase):
    def test_both_runner_formats_and_balanced_orders_are_supported(self):
        for metric in ["fixed-time", "original-score"]:
            for strategy, repeat in [("abba", 6), ("baab", 6), ("abba-baab", 8)]:
                with self.subTest(metric=metric, strategy=strategy):
                    report = paired.paired_report(fixture(metric, repeat=repeat, order=strategy), "base", "new", metric)
                    self.assertEqual(report["results"][0]["pairs"], repeat)
                    self.assertAlmostEqual(report["results"][0]["median_gain_percent"], 10)

    def test_missing_duplicate_and_wrong_identity_cannot_be_silently_dropped(self):
        for mutation, message in [
            (lambda x: x["samples"].pop(), "missing pairs"),
            (lambda x: x["samples"].append(x["samples"][0]), "duplicate"),
            (lambda x: x["samples"][0].update(repetition=6), "outside"),
            (lambda x: x["samples"][0].update(repetition=True), "outside"),
            (lambda x: x["samples"][0].update(engine="foreign"), "outside"),
            (lambda x: x["samples"][0].update(case="foreign"), "outside"),
        ]:
            result = fixture()
            mutation(result)
            with self.subTest(message=message), self.assertRaisesRegex(ValueError, message):
                paired.paired_report(result, "base", "new", "fixed-time")

    def test_recorded_execution_order_must_match_declared_balance(self):
        result = fixture()
        result["samples"][0], result["samples"][1] = result["samples"][1], result["samples"][0]
        with self.assertRaisesRegex(ValueError, "journal.*balanced order"):
            paired.paired_report(result, "base", "new", "fixed-time")
        for repeat, order in [(4, "abba"), (7, "abba"), (6, "default")]:
            result = fixture()
            result["metadata"].update(repeat=repeat, order_strategy=order)
            with self.subTest(repeat=repeat, order=order), self.assertRaises(ValueError):
                paired.paired_report(result, "base", "new", "fixed-time")

    def test_one_failed_or_unverified_sample_rejects_whole_report(self):
        for changes in [dict(status="failed"), dict(exit_code=1), dict(timed_out=True),
                        dict(semantic_status="wrong-output"), dict(process_wall_ns=0),
                        dict(workload_sha256="f" * 64)]:
            result = fixture()
            result["samples"][0].update(changes)
            with self.subTest(changes=changes), self.assertRaises(ValueError):
                paired.paired_report(result, "base", "new", "fixed-time")

    def test_score_results_cannot_use_fixed_time_noise_or_incomplete_scores(self):
        result = fixture("original-score")
        del result["samples"][0]["measurements"]["Richards"]
        with self.assertRaisesRegex(ValueError, "missing or unexpected original scores"):
            paired.paired_report(result, "base", "new", "original-score")
        with self.assertRaisesRegex(ValueError, "original v8-v7 runner"):
            paired.paired_report(fixture(), "base", "new", "original-score")

    def test_contextual_combined_subscores_are_separate(self):
        result = fixture("original-score", ["richards", "all"])
        result["workloads"][1]["expected"] = ["Richards", "DeltaBlue"]
        for sample in result["samples"]:
            if sample["case"] == "all":
                score = sample["measurements"]["Score"]
                sample["measurements"] = dict(Score=score, Richards=score * 2, DeltaBlue=score * 3)
        report = paired.paired_report(result, "base", "new", "original-score")
        self.assertEqual([(r["case"], r["measurement"]) for r in report["results"]],
                         [("richards", "Score"), ("all", "Score")])
        self.assertEqual([(r["case"], r["measurement"]) for r in report["combined_subscores"]],
                         [("all", "Richards"), ("all", "DeltaBlue")])

    def test_full_acceptance_requires_every_isolated_case_and_combined(self):
        with self.assertRaisesRegex(ValueError, "eight isolated cases"):
            paired.paired_report(fixture(), "base", "new", "fixed-time", require_v8_matrix=True)
        result = fixture(cases=sorted(paired.V8_ISOLATED_CASES) + ["combined"])
        report = paired.paired_report(result, "base", "new", "fixed-time", require_v8_matrix=True)
        self.assertEqual(len(report["results"]), 9)
        original = fixture("original-score", cases=sorted(paired.V8_ISOLATED_CASES) + ["all"])
        report = paired.paired_report(original, "base", "new", "original-score", require_v8_matrix=True)
        self.assertEqual(len(report["results"]), 9)
        self.assertEqual(len(report["combined_subscores"]), 8)
        original["workloads"][-1]["expected"] = ["Richards"]
        for sample in original["samples"]:
            if sample["case"] == "all":
                sample["measurements"] = {key: value for key, value in sample["measurements"].items()
                                          if key in ("Score", "Richards")}
        with self.assertRaisesRegex(ValueError, "eight original score names"):
            paired.paired_report(original, "base", "new", "original-score", require_v8_matrix=True)


class GateTests(unittest.TestCase):
    def test_gain_must_exceed_frozen_matching_noise(self):
        aa = fixture(gains=[0.02] * 6, same_binary=True)
        for gain, passed in [(0.01, False), (0.02, False), (0.03, True)]:
            report = paired.paired_report(fixture(gains=[gain] * 6), "base", "new", "fixed-time",
                                          aa_results=aa, gate="net-gain")
            self.assertEqual(report["gate_passed"], passed)
            self.assertAlmostEqual(report["results"][0]["aa"]["noise_percent"], 2)

    def test_noninferiority_has_fixed_margin_and_reports_slowdown(self):
        aa = fixture(gains=[0.03] * 6, same_binary=True)
        for gain, passed in [(-0.005, True), (-0.02, False)]:
            report = paired.paired_report(fixture(gains=[gain] * 6), "base", "new", "fixed-time",
                                          aa_results=aa, gate="noninferior")
            self.assertEqual(report["gate_passed"], passed)
            self.assertEqual(report["noninferiority_margin_percent"], 1)
            self.assertLess(report["results"][0]["median_gain_percent"], 0)

    def test_positive_median_with_uncertain_interval_does_not_pass(self):
        report = paired.paired_report(fixture(gains=[-.1, -.1, .01, .01, .01, .01]),
                                      "base", "new", "fixed-time",
                                      aa_results=fixture(gains=[0] * 6, same_binary=True),
                                      gate="net-gain")
        self.assertGreater(report["results"][0]["median_gain_percent"], 0)
        self.assertLess(report["results"][0]["gain_ci95_percent"][0], 0)
        self.assertFalse(report["gate_passed"])

    def test_noise_requires_same_binary_identical_workload_and_metric_settings(self):
        for mutation, message in [
            (lambda x: x["metadata"]["engines"]["new"].update(sha256="b" * 64), "same binary"),
            (lambda x: x["metadata"]["workloads"][0].update(sha256="c" * 64), "bytes or metric"),
            (lambda x: x["metadata"].update(cpu=3), "settings differ"),
            (lambda x: x["metadata"]["machine"].update(cpu="other-cpu"), "settings differ"),
        ]:
            aa = fixture(gains=[0] * 6, same_binary=True)
            mutation(aa)
            with self.subTest(message=message), self.assertRaisesRegex(ValueError, message):
                paired.paired_report(fixture(), "base", "new", "fixed-time", aa_results=aa)
        with self.assertRaisesRegex(ValueError, "frozen matching A/A"):
            paired.paired_report(fixture(), "base", "new", "fixed-time", gate="net-gain")

    def test_aa_full_matrix_can_supply_noise_for_selected_workloads(self):
        aa = fixture(cases=["richards", "deltablue"], gains=[0] * 6, same_binary=True)
        report = paired.paired_report(fixture(), "base", "new", "fixed-time", aa_results=aa, gate="net-gain")
        self.assertTrue(report["gate_passed"])

    def test_cli_writes_evidence_and_returns_failure_for_unpassed_gate(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            results = root / "results.json"
            aa = root / "aa.json"
            output = root / "paired.json"
            results.write_text(json.dumps(fixture(gains=[0] * 6)))
            aa.write_text(json.dumps(fixture(gains=[0] * 6, same_binary=True)))
            arguments = ["paired.py", "--results", str(results), "--baseline", "base", "--candidate", "new",
                         "--metric", "fixed-time", "--aa-results", str(aa), "--gate", "net-gain", "--output", str(output)]
            with mock.patch.object(sys, "argv", arguments):
                self.assertEqual(paired.main(), 1)
            report = json.loads(output.read_text())
            self.assertEqual(report["inputs"]["results"]["sha256"], digest(results))
            self.assertFalse(report["gate_passed"])
            self.assertIn("not passed", output.with_suffix(".md").read_text())
            with mock.patch.object(sys, "argv", arguments), mock.patch.object(sys, "stderr"), self.assertRaises(SystemExit):
                paired.main()


if __name__ == "__main__":
    unittest.main()
