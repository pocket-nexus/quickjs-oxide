"""The memory table must read the reports `qjs` prints and refuse a run without them."""
import unittest
from memory_table import geomean, markdown, parse_reports

REPORT = """\
gc: 9 collections, total pause 719.8 ms, p50 102.449 ms, p99 141.841 ms, max 141.841 ms, live 76839 KiB, reserved 300807 KiB
memory            live  capacity          used      reserved
objects        1381869   2042462   26989.6 KiB   49664.0 KiB
heap reserved: 300807.7 KiB; threshold 153679.8 KiB; cap none
code: 45 units, 1222 instructions (9.5 KiB), positions 7.7 KiB, source 12.8 KiB, 0 regexp programs
"""


class ReportTests(unittest.TestCase):
    def test_reports_are_parsed(self):
        figures = parse_reports(REPORT)
        self.assertEqual(figures["collections"], 9)
        self.assertEqual(figures["live_kib"], 76839)
        self.assertEqual(figures["reserved_kib"], 300807)
        self.assertEqual(figures["threshold_kib"], 153679.8)
        self.assertIsNone(figures["budget_kib"])
        self.assertEqual(figures["code_units"], 45)
        self.assertAlmostEqual(figures["code_kib"], 30.0)

    def test_dynamic_code_is_read_when_printed(self):
        report = REPORT.replace("code: 45 units,", "code: 45 units (40 static, 5 dynamic holding 2.5 KiB),")
        figures = parse_reports(report)
        self.assertEqual(figures["code_units"], 45)
        self.assertEqual(figures["code_dynamic_kib"], 2.5)
        self.assertAlmostEqual(figures["code_kib"], 30.0)

    def test_budget_is_read_when_printed(self):
        report = REPORT.replace("threshold 153679.8 KiB;", "threshold 153679.8 KiB; budget 160000.0 KiB;")
        self.assertEqual(parse_reports(report)["budget_kib"], 160000.0)

    def test_a_missing_report_is_an_error(self):
        with self.assertRaisesRegex(ValueError, "gc-stats"):
            parse_reports(REPORT.splitlines()[1])

    def test_table_ends_with_the_geometric_mean(self):
        row = {"program": "awfy-Bounce", "qjs_kib": 8000, "quickjs_kib": 4000, "ratio": 2.0, "collections": 2,
               "live_kib": 290, "reserved_kib": 1024, "budget_kib": None, "code_kib": 483.0}
        table = markdown({"qjs": 4000, "quickjs": 3000}, [row, {**row, "ratio": 0.5}])
        self.assertIn("| awfy-Bounce | 8,000 | 4,000 | 2.00 | 2 | 290 | 1,024 | 483 |", table)
        self.assertIn("Empty script: 4,000 / 3,000 KiB.", table)
        self.assertTrue(table.endswith("over 2 programs: 1.000\n"))
        self.assertAlmostEqual(geomean([2.0, 8.0]), 4.0)


if __name__ == "__main__":
    unittest.main()
