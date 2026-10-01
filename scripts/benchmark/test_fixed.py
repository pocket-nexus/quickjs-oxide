"""Manifest admission must reject stale or ambiguous fixed-work experiments."""
import json
import tempfile
import unittest
from pathlib import Path
from fixed import load_workloads, parse_darwin_counters
from run import digest


class ManifestTests(unittest.TestCase):
    def test_darwin_time_l_counter_fixture(self):
        raw = ("0.01 real 0.01 user 0.00 sys\n"
               "       123,456 instructions retired\n"
               "       9,876 cycles elapsed\n"
               "       2097152 maximum resident set size\n"
               "       1048576 peak memory footprint\n")
        self.assertEqual(parse_darwin_counters(raw), {
            'instructions_retired': 123456, 'cycles_elapsed': 9876,
            'maximum_resident_set_size': 2097152, 'peak_memory_footprint': 1048576,
        })
        with self.assertRaisesRegex(ValueError, 'missing Darwin counters'):
            parse_darwin_counters(raw.replace('9,876 cycles elapsed\n', ''))
        with self.assertRaisesRegex(ValueError, 'must be positive'):
            parse_darwin_counters(raw.replace('9,876 cycles elapsed', '0 cycles elapsed'))

    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.directory = Path(self.temp.name)
        self.source = self.directory / 'loop.js'
        self.source.write_text('print(42);\n')
        self.item = dict(case='loop', path='/missing/loop.js', sha256=digest(self.source),
                         size=1, expected='42\n')
        self.report = self.directory / 'report.json'

    def write(self, items):
        self.report.write_text(json.dumps(dict(metadata=dict(workloads=dict(workloads=items)))))

    def test_relocation_preserves_byte_identity(self):
        self.write([self.item])
        result = load_workloads(self.report, self.directory, ['loop'])
        self.assertEqual(Path(result[0]['path']), self.source.resolve())
        self.assertEqual(result[0]['expected'], '42\n')

    def test_changed_workload_is_rejected(self):
        self.write([self.item])
        self.source.write_text('print(41);\n')
        with self.assertRaisesRegex(ValueError, 'bytes changed'):
            load_workloads(self.report, self.directory)

    def test_missing_or_repeated_selection_is_rejected(self):
        self.write([self.item])
        for cases in [['missing'], ['loop', 'loop']]:
            with self.subTest(cases=cases), self.assertRaises(ValueError):
                load_workloads(self.report, self.directory, cases)

    def test_ambiguous_or_unsafe_manifest_is_rejected(self):
        for items in [[self.item, self.item], [dict(self.item, case='../loop')], []]:
            self.write(items)
            with self.subTest(items=items), self.assertRaises(ValueError):
                load_workloads(self.report, self.directory)
