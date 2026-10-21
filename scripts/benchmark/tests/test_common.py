"""Shared process sampling, machine metadata and balanced pair order."""
from pathlib import Path
import sys
import tempfile
import unittest
from unittest import mock

from oxbench import common


class CommonTests(unittest.TestCase):
    def test_macos_machine_metadata_is_read_only_and_specific(self):
        def command_output(command, _cwd=None):
            values = {("sysctl", "-n", "machdep.cpu.brand_string"): "Apple M1",
                      ("sysctl", "-n", "hw.memsize"): "17179869184",
                      ("pmset", "-g", "custom"): "AC Power:\n lowpowermode 0",
                      ("pmset", "-g", "batt"): "Now drawing from 'AC Power'",
                      ("vm_stat",): "Pages free: 100."}
            return {"exit_code": 0, "stdout": values[tuple(command)], "stderr": "", "command": command}

        with mock.patch.object(common.platform, "system", return_value="Darwin"), \
             mock.patch.object(common, "command_output", side_effect=command_output), \
             mock.patch.object(common, "git_metadata", return_value={}), \
             mock.patch.object(common.os, "getloadavg", return_value=(1.0, 2.0, 3.0)):
            result = common.machine_metadata()
        self.assertEqual(result["cpu"], "Apple M1")
        self.assertEqual(result["macos"]["physical_memory_bytes"], 17179869184)
        self.assertIn("lowpowermode 0", result["macos"]["power_settings"]["stdout"])
        self.assertEqual(result["load_average"], [1.0, 2.0, 3.0])

    def test_explicit_paired_orders_are_balanced(self):
        engines = ["base", "candidate"]
        self.assertEqual([name for repetition in range(4)
                          for name in common.paired_order(engines, repetition, "abba-baab")],
                         ["base", "candidate", "candidate", "base",
                          "candidate", "base", "base", "candidate"])
        self.assertEqual([name for repetition in range(2)
                          for name in common.paired_order(engines, repetition, "baab")],
                         ["candidate", "base", "base", "candidate"])
        for names, repeat, mode in [(engines, 3, "abba"), (engines, 2, "abba-baab"),
                                    (["one"], 2, "abba")]:
            with self.subTest(names=names, repeat=repeat, mode=mode), self.assertRaises(ValueError):
                common.validate_paired_order(names, repeat, mode)

    def test_timeout_is_retained_with_raw_output(self):
        with tempfile.TemporaryDirectory() as directory:
            prefix = Path(directory) / "sample"
            result = common.run_sample([sys.executable, "-c", "import time; print('started', flush=True); time.sleep(2)"], directory, prefix, .2)
            self.assertTrue(result["timed_out"])
            self.assertNotEqual(result["exit_code"], 0)
            self.assertIn("started", prefix.with_suffix(".stdout").read_text())


if __name__ == "__main__":
    unittest.main()
