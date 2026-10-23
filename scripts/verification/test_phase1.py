"""Keep the proof receipt gate closed on incomplete or vacuous verification."""
import argparse
import copy
import unittest

from phase1 import HARNESSES, VERSION, check_results, harness_timeout


def successful_report():
    return {
        "metadata": {"kani_version": VERSION, "target": "x86_64-unknown-linux-gnu"},
        "verification_results": {
            "summary": {"status": "completed", "total_harnesses": len(HARNESSES),
                        "executed": len(HARNESSES), "successful": len(HARNESSES), "failed": 0},
            "results": [{"harness_id": name, "status": "Success", "checks": [{
                "status": "Success", "description": "assertion failed: property",
                "location": {"file": "core/src/value_kani_tests.rs"},
            }]} for name in sorted(HARNESSES)],
        },
    }


class ReceiptTests(unittest.TestCase):
    def test_timeout_requires_a_positive_kani_duration(self):
        for duration in ('60', '1800s', '30m', '1h'):
            self.assertEqual(harness_timeout(duration), duration)
        for duration in ('0', '0s', '-1h', '1.5h', 'forever', ''):
            with self.subTest(duration=duration), self.assertRaises(argparse.ArgumentTypeError):
                harness_timeout(duration)

    def test_accepts_success_and_unreachable_internal_checks(self):
        report = successful_report()
        report["verification_results"]["results"][0]["checks"].append({
            "status": "Unreachable", "description": "attempt to add with overflow",
            "location": {"file": "core/src/heap/values.rs"},
        })
        check_results(report)

    def test_rejects_missing_duplicate_failed_and_vacuous_harnesses(self):
        for fault in ("missing", "duplicate", "failed", "unreachable", "no_assertions", "unknown"):
            with self.subTest(fault=fault):
                report = successful_report()
                results = report["verification_results"]["results"]
                if fault == "missing":
                    results.pop()
                elif fault == "duplicate":
                    results[0] = copy.deepcopy(results[1])
                elif fault == "failed":
                    results[0]["status"] = "Failure"
                elif fault == "unreachable":
                    results[0]["checks"][0]["status"] = "Unreachable"
                elif fault == "no_assertions":
                    results[0]["checks"] = []
                else:
                    results[0]["checks"][0]["status"] = "Undetermined"
                with self.assertRaises(ValueError):
                    check_results(report)

    def test_rejects_partial_run_and_wrong_configuration(self):
        for field, value in (("target", "aarch64-apple-darwin"), ("kani_version", "0.1")):
            report = successful_report()
            report["metadata"][field] = value
            with self.assertRaises(ValueError):
                check_results(report)
        report = successful_report()
        report["verification_results"]["summary"]["executed"] = 0
        with self.assertRaises(ValueError):
            check_results(report)


if __name__ == "__main__":
    unittest.main()
