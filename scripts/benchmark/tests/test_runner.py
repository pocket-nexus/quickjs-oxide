"""Manifest admission must reject stale or ambiguous fixed-work experiments."""
import contextlib
import io
import json
import tempfile
import unittest
from pathlib import Path
from unittest import mock

from oxbench import runner
from oxbench.runner import load_workloads, parse_darwin_counters, parse_linux_counters
from oxbench.common import digest


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

    def test_linux_perf_stat_counter_fixture(self):
        raw = ("# started on Fri Oct 16 08:00:00 2026\n\n"
               "1234.56,msec,task-clock,1234560000,100.00,0.999,CPUs utilized\n"
               "5678901234,,cycles:u,1234000000,100.00,,\n"
               "9876543210,,instructions:u,1234000000,100.00,1.74,insn per cycle\n")
        self.assertEqual(parse_linux_counters(raw), {
            'task_clock_ms': 1234.56, 'cycles': 5678901234, 'instructions': 9876543210})
        self.assertEqual(parse_linux_counters(raw.replace(',task-clock,', ',task-clock:u,'))['task_clock_ms'],
                         1234.56)
        with self.assertRaisesRegex(ValueError, 'not counted'):
            parse_linux_counters(raw.replace('5678901234,,cycles:u', '<not counted>,,cycles:u'))
        with self.assertRaisesRegex(ValueError, 'missing Linux counters'):
            parse_linux_counters(raw.replace('instructions:u', 'branches:u'))

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


class MeasurementTests(unittest.TestCase):
    def fixture(self, root):
        source = root / "workload.js"
        source.write_text("print(42);\n")
        engine = root / "engine"
        engine.write_text("frozen engine\n")
        engine.chmod(0o755)
        manifest = root / "manifest.json"
        manifest.write_text(json.dumps({"metadata": {"workloads": {"workloads": [
            dict(case="loop", path=str(source), sha256=digest(source), size=1, expected="42\n")]}}}))
        return source, engine, manifest

    @staticmethod
    def sample(command, cwd, prefix, timeout):
        stdout, stderr = prefix.with_suffix(".stdout"), prefix.with_suffix(".stderr")
        stdout.write_text("42\n")
        stderr.write_text("")
        return dict(command=command, exit_code=0, timed_out=False, process_wall_ns=100,
                    stdout=str(stdout), stderr=str(stderr))

    def test_cpu_affinity_wraps_measured_process_and_is_recorded(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source, engine, manifest = self.fixture(root)
            commands = []

            def sample(command, *rest):
                commands.append(command)
                return self.sample(command, *rest)

            output = root / "results"
            argv = ["--manifest", str(manifest), "--engine", f"base={engine}", "--repeat", "1",
                    "--cpu", "2", "--output", str(output)]
            with mock.patch.object(runner, "machine_metadata", return_value={}), \
                 mock.patch.object(runner, "binary_metadata", side_effect=lambda path: {"sha256": digest(path)}), \
                 mock.patch.object(runner, "run_sample", side_effect=sample), \
                 contextlib.redirect_stdout(io.StringIO()):
                self.assertEqual(runner.main(argv), 0)
            self.assertEqual(commands, [["taskset", "-c", "2", str(engine.resolve()), str(source.resolve())]])
            self.assertEqual(json.loads((output / "metadata.json").read_text())["cpu"], 2)

    def test_changed_inputs_abort_before_next_sample(self):
        for changed, message in (("workload", "workload changed"), ("engine", "binary changed")):
            with self.subTest(changed=changed), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                source, engine, manifest = self.fixture(root)
                calls = []

                def sample(command, *rest):
                    calls.append(command)
                    path = source if changed == "workload" else engine
                    path.write_bytes(path.read_bytes() + b"// changed\n")
                    return self.sample(command, *rest)

                argv = ["--manifest", str(manifest), "--engine", f"candidate={engine}", "--repeat", "2",
                        "--output", str(root / "results")]
                with mock.patch.object(runner, "machine_metadata", return_value={}), \
                     mock.patch.object(runner, "binary_metadata", side_effect=lambda path: {"sha256": digest(path)}), \
                     mock.patch.object(runner, "run_sample", side_effect=sample), \
                     contextlib.redirect_stdout(io.StringIO()):
                    with self.assertRaisesRegex(ValueError, message):
                        runner.main(argv)
                self.assertEqual(len(calls), 1)
