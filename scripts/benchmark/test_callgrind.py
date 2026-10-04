"""Counter admission cannot replace guest output and workload authentication."""
import contextlib
import io
import json
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import callgrind
import fixed
from run import digest


CACHES = {'I1': [32768, 8, 64], 'D1': [32768, 8, 64], 'LL': [16777216, 1, 64]}
PROFILE = '''# callgrind format
version: 1
desc: I1 cache: 32768 B, 64 B, 8-way associative
desc: D1 cache: 32768 B, 64 B, 8-way associative
desc: LL cache: 16777216 B, 64 B, direct-mapped
events: Ir Dr Dw I1mr D1mr D1mw Bc Bcm Bi Bim
summary: 102 20 30 4 5 6 7 8 9 10
totals: 100 20 30 4 5 6 7 8 9 10
'''


class CounterTests(unittest.TestCase):
    def test_final_totals_and_event_names_are_used(self):
        self.assertEqual(callgrind.parse_profile(PROFILE, CACHES)['Ir'], 100)
        reordered = PROFILE.replace('Bc Bcm Bi Bim', 'Bim Bi Bcm Bc')
        self.assertEqual(callgrind.parse_profile(reordered, CACHES)['Bim'], 7)

    def test_partial_ambiguous_or_invalid_profiles_are_rejected(self):
        profiles = [PROFILE.split('totals:')[0], PROFILE + 'totals: 1\n',
                    PROFILE.replace('Bi Bim', 'Bi Bi'),
                    PROFILE.replace('totals: 100', 'totals: -100'),
                    PROFILE.replace('totals: 100', 'totals: 0'),
                    PROFILE.replace('D1 cache: 32768', 'D1 cache: 65536')]
        for raw in profiles:
            with self.subTest(raw=raw), self.assertRaises(ValueError):
                callgrind.parse_profile(raw, CACHES)

    def test_instrumented_times_are_not_speed_summary(self):
        sample = dict(case='test', engine='A', status='ok', process_wall_ns=123,
                      callgrind={'values': callgrind.parse_profile(PROFILE, CACHES)})
        row = callgrind.summarize([sample])[0]
        self.assertEqual(row['median_counters']['Ir'], 100)
        self.assertNotIn('median_wall_ns', row)
        sample['status'] = 'unexpected-stderr'
        self.assertIsNone(callgrind.summarize([sample])[0]['median_counters'])

    def test_cache_config_rejects_boolean_and_invalid_geometry(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'cache.json'
            for config in [dict(CACHES, I1=[True, 8, 64]), dict(CACHES, D1=[32769, 8, 64]),
                           dict(CACHES, unknown=[32768, 8, 64])]:
                path.write_text(json.dumps(config))
                with self.subTest(config=config), self.assertRaises(ValueError):
                    callgrind.load_cache_config(path)

    def replay_fixture(self, output='42\n', profile=PROFILE):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        directory = Path(temp.name)
        config = directory / 'cache.json'
        config.write_text(json.dumps(CACHES))
        engine = directory / 'qjs'
        engine.write_text('#!/usr/bin/env python3\nprint(42)\n')
        engine.chmod(0o755)
        tool = directory / 'valgrind'
        tool.write_text('#!/usr/bin/env python3\nimport sys\nfrom pathlib import Path\n'
                        'if "--version" in sys.argv:\n print("valgrind-test");sys.exit(0)\n'
                        'for arg in sys.argv[1:]:\n'
                        ' if arg.startswith("--callgrind-out-file="):\n'
                        f'  Path(arg.split("=",1)[1]).write_text({profile!r})\n'
                        ' if arg.startswith("--log-file="):\n'
                        '  Path(arg.split("=",1)[1]).write_text("profiler log\\n")\n'
                        f'sys.stdout.write({output!r})\n')
        tool.chmod(0o755)
        source = directory / 'loop.js'
        source.write_text('print(42);\n')
        manifest = directory / 'manifest.json'
        manifest.write_text(json.dumps({'metadata': {'workloads': {'workloads': [
            dict(case='test', path=str(source), sha256=digest(source), size=1, expected='42\n')
        ]}}}))
        result = directory / 'result'
        args = ['fixed.py', '--manifest', str(manifest), '--engine', 'A=' + str(engine),
                '--repeat', '1', '--callgrind', str(tool), '--callgrind-cache-config', str(config),
                '--output', str(result)]
        with patch.dict('os.environ', {'VALGRIND_LIB': ''}), patch.object(sys, 'argv', args), \
                contextlib.redirect_stdout(io.StringIO()):
            code = fixed.main()
        return code, json.loads((result / 'results.json').read_text())

    def test_complete_replay_keeps_js_stderr_separate(self):
        code, result = self.replay_fixture()
        self.assertEqual(code, 0)
        sample = result['samples'][0]
        self.assertEqual(Path(sample['stderr']).read_bytes(), b'')
        self.assertEqual(sample['callgrind']['values']['Dw'], 30)
        self.assertNotIn('median_wall_ns', result['summary'][0])

    def test_wrong_guest_output_fails_even_with_valid_counters(self):
        code, result = self.replay_fixture(output='41\n')
        self.assertEqual(code, 1)
        self.assertNotEqual(result['samples'][0]['semantic_status'], 'ok')

    def test_partial_profile_fails_even_with_correct_guest_output(self):
        code, result = self.replay_fixture(profile=PROFILE.split('totals:')[0])
        self.assertEqual(code, 1)
        self.assertEqual(result['samples'][0]['status'], 'invalid-callgrind-counters')


if __name__ == '__main__':
    unittest.main()
