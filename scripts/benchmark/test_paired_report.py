"""Balanced process blocks are the unit of inference, not guest iterations."""

import copy
import math
import unittest

import paired_report


def fixture(left, right, left_hash, right_hash):
    samples = []
    for repetition in range(4):
        for engine, wall in ((left, 100), (right, (90, 110, 80, 120)[repetition])):
            samples.append(dict(case='app', repetition=repetition, engine=engine,
                                process_wall_ns=wall, status='ok', semantic_status='ok'))
    return dict(metadata=dict(manifest_sha256='manifest', repeat=4,
                              order_strategy='abba-baab',
                              engines={left: dict(sha256=left_hash),
                                       right: dict(sha256=right_hash)}),
                samples=samples)


class PairedReportTests(unittest.TestCase):
    def setUp(self):
        self.ab = fixture('baseline', 'integrated', 'base-hash', 'final-hash')
        self.aa = fixture('left', 'right', 'final-hash', 'final-hash')

    def analyze(self):
        return paired_report.analyze(self.ab, self.aa, 'baseline', 'integrated',
                                     'left', 'right')

    def test_report_uses_complete_process_blocks(self):
        report = self.analyze()
        ab = report['cases']['app']['ab']
        self.assertEqual(ab['blocks'], 2)
        self.assertAlmostEqual(ab['block_log_ratios'][0],
                               (math.log(.9) + math.log(1.1)) / 2)
        self.assertAlmostEqual(ab['geometric_ratio'],
                               (.9 * 1.1 * .8 * 1.2) ** .25)
        self.assertEqual(report['cases']['app']['aa']['geometric_ratio'],
                         ab['geometric_ratio'])

    def test_rejects_failed_or_missing_process(self):
        self.ab['samples'][0]['semantic_status'] = 'wrong'
        with self.assertRaisesRegex(ValueError, 'invalid process sample'):
            self.analyze()
        self.ab['samples'][0]['semantic_status'] = 'ok'
        self.ab['samples'].pop()
        with self.assertRaisesRegex(ValueError, 'missing or unexpected'):
            self.analyze()

    def test_rejects_mismatched_work_or_candidate_binary(self):
        self.aa['metadata']['manifest_sha256'] = 'different'
        with self.assertRaisesRegex(ValueError, 'different workload manifests'):
            self.analyze()
        self.aa['metadata']['manifest_sha256'] = 'manifest'
        self.aa['metadata']['engines']['right']['sha256'] = 'different'
        with self.assertRaisesRegex(ValueError, 'binaries do not match'):
            self.analyze()

    def test_rejects_unbalanced_or_duplicate_schedule(self):
        changed = copy.deepcopy(self.ab)
        changed['metadata']['order_strategy'] = 'default'
        with self.assertRaisesRegex(ValueError, 'balanced'):
            paired_report.blocks(changed, 'baseline', 'integrated')
        self.ab['samples'].append(dict(self.ab['samples'][0]))
        with self.assertRaisesRegex(ValueError, 'duplicate'):
            self.analyze()


if __name__ == '__main__':
    unittest.main()
