"""Replay a recorded fixed-work matrix, verifying bytes before measuring."""
import argparse
import json
import os
import platform
import re
from pathlib import Path
from .common import binary_metadata, digest, machine_metadata, paired_order, run_sample, validate_paired_order
from .scaling import admit, summarize
from . import callgrind


DARWIN_COUNTERS = {
    'instructions retired': 'instructions_retired',
    'cycles elapsed': 'cycles_elapsed',
    'maximum resident set size': 'maximum_resident_set_size',
    'peak memory footprint': 'peak_memory_footprint',
}


def parse_darwin_counters(raw):
    """Parse exact whole-process fields from macOS /usr/bin/time -l output."""
    values = {}
    for line in raw.splitlines():
        match = re.fullmatch(r'\s*([\d,]+)\s+(.+?)\s*', line)
        if match and match[2] in DARWIN_COUNTERS:
            key = DARWIN_COUNTERS[match[2]]
            if key in values:
                raise ValueError(f'duplicate Darwin counter: {key}')
            values[key] = int(match[1].replace(',', ''))
    if len(values) != len(DARWIN_COUNTERS):
        raise ValueError('missing Darwin counters: ' + ', '.join(sorted(set(DARWIN_COUNTERS.values()) - set(values))))
    if any(value <= 0 for value in values.values()):
        raise ValueError('Darwin counters must be positive for a measured process')
    return values


LINUX_EVENTS = ('instructions:u', 'cycles:u', 'task-clock')


def parse_linux_counters(raw):
    """Parse `perf stat -x,` output for the user-space events of one process."""
    values = {}
    for line in raw.splitlines():
        fields = line.split(',')
        if len(fields) < 3:
            continue
        # perf reports task-clock as task-clock:u when the other events are user-only.
        event = 'task-clock' if fields[2] == 'task-clock:u' else fields[2]
        if event not in LINUX_EVENTS:
            continue
        key = {'instructions:u': 'instructions', 'cycles:u': 'cycles', 'task-clock': 'task_clock_ms'}[event]
        if key in values:
            raise ValueError(f'duplicate Linux counter: {key}')
        try:
            values[key] = float(fields[0]) if key == 'task_clock_ms' else int(fields[0])
        except ValueError:
            raise ValueError(f'Linux counter {key} not counted: {fields[0]}') from None
    if len(values) != len(LINUX_EVENTS):
        raise ValueError('missing Linux counters')
    if any(value <= 0 for value in values.values()):
        raise ValueError('Linux counters must be positive for a measured process')
    return values


def load_workloads(report, directory=None, cases=None):
    workloads = json.loads(Path(report).read_text())["metadata"]["workloads"]["workloads"]
    names = set()
    for item in workloads:
        name = item["case"]
        if not re.fullmatch(r"[a-zA-Z0-9_-]+", name) or name in names:
            raise ValueError("workload names must be unique safe identifiers")
        names.add(name)
    if cases and (len(cases) != len(set(cases)) or set(cases) - names):
        raise ValueError("requested cases must be unique and present in the manifest")
    selected = []
    for item in workloads:
        if cases and item["case"] not in cases:
            continue
        item = dict(item)
        path = (Path(directory) / Path(item["path"]).name) if directory else Path(item["path"])
        if digest(path) != item["sha256"]:
            raise ValueError(f"workload bytes changed: {item['case']}")
        item["path"] = str(path.resolve())
        selected.append(item)
    if not selected:
        raise ValueError("empty fixed-work matrix")
    return selected


def main(argv=None):
    parser = argparse.ArgumentParser(prog="bench.py run", description=__doc__)
    parser.add_argument('--manifest', type=Path, required=True)
    parser.add_argument('--workload-dir', type=Path)
    parser.add_argument('--engine', action='append', required=True)
    parser.add_argument('--case', action='append')
    parser.add_argument('--repeat', type=int, default=5)
    parser.add_argument('--order', choices=['default', 'abba', 'baab', 'abba-baab'], default='default',
                        help='two-engine balanced order; default preserves rotating engine order')
    parser.add_argument('--timeout', type=float, default=180)
    parser.add_argument('--cpu', type=int)
    parser.add_argument('--darwin-counters', action='store_true',
                        help='wrap each sample in /usr/bin/time -l and retain whole-process counters')
    parser.add_argument('--linux-counters', action='store_true',
                        help='wrap each sample in perf stat and retain user-space instructions and cycles')
    parser.add_argument('--callgrind', type=Path, help='Valgrind executable; instrument fixed work instead of timing it')
    parser.add_argument('--callgrind-cache-config', type=Path,
                        help='frozen JSON with I1/D1/LL [bytes, associativity, line_bytes]')
    parser.add_argument('--callgrind-instructions', action='store_true',
                        help='retain instruction positions for code-generation attribution')
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args(argv)
    if args.repeat < 1 or args.timeout <= 0 or (args.cpu is not None and args.cpu < 0):
        parser.error('invalid repeat, timeout or CPU')
    if args.darwin_counters and (platform.system() != 'Darwin' or not os.access('/usr/bin/time', os.X_OK)):
        parser.error('--darwin-counters requires macOS /usr/bin/time')
    if args.darwin_counters and args.cpu is not None:
        parser.error('--cpu uses taskset and cannot be combined with --darwin-counters')
    if args.linux_counters and (platform.system() != 'Linux' or args.darwin_counters or args.callgrind):
        parser.error('--linux-counters requires Linux perf and excludes the other counter modes')
    if bool(args.callgrind) != bool(args.callgrind_cache_config):
        parser.error('--callgrind and --callgrind-cache-config must be used together')
    if args.callgrind_instructions and not args.callgrind:
        parser.error('--callgrind-instructions requires --callgrind')
    if args.callgrind and args.darwin_counters:
        parser.error('Callgrind and native Darwin counters are separate measurements')
    engines = {}
    for entry in args.engine:
        name, separator, path = entry.partition('=')
        if not separator or not re.fullmatch(r'[a-zA-Z0-9_-]+', name) or name in engines:
            parser.error('engines must be unique name=path entries')
        engines[name] = Path(path).resolve()
    try:
        validate_paired_order(list(engines), args.repeat, args.order)
    except ValueError as error:
        parser.error(str(error))
    workloads = load_workloads(args.manifest, args.workload_dir, args.case)
    instrument = callgrind.tool_metadata(args.callgrind, args.callgrind_cache_config,
                                        args.callgrind_instructions) if args.callgrind else None
    metadata = dict(
        machine=machine_metadata(),
        runner_sha256=digest(__file__),
        manifest_sha256=digest(args.manifest),
        workloads=workloads,
        engines={name: binary_metadata(path) for name, path in engines.items()},
        repeat=args.repeat,
        order_strategy=args.order,
        timeout_seconds=args.timeout,
        cpu=args.cpu,
        darwin_counters=args.darwin_counters,
        linux_counters=list(LINUX_EVENTS) if args.linux_counters else None,
        counter_scope='whole process including startup and teardown'
        if args.darwin_counters or args.linux_counters else None,
        callgrind=instrument,
        metric='Callgrind whole-process event counts; instrumented times are not speed evidence'
        if instrument else 'whole-process wall nanoseconds; not adaptive scores',
    )
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    (output / 'raw').mkdir()
    (output / 'metadata.json').write_text(json.dumps(metadata, indent=2) + '\n')
    samples = []
    with (output / 'samples.jsonl').open('w') as journal:
        for workload in workloads:
            for repetition in range(args.repeat):
                names = list(engines)
                offset = repetition % len(names)
                order = names[offset:] + names[:offset] if args.order == 'default' else paired_order(names, repetition, args.order)
                for name in order:
                    if digest(workload['path']) != workload['sha256']:
                        raise ValueError('workload changed during measurement')
                    if digest(engines[name]) != metadata['engines'][name]['sha256']:
                        raise ValueError('binary changed during measurement')
                    prefix = output / 'raw' / f"{workload['case']}-{name}-{repetition}"
                    cmd = [str(engines[name]), workload['path']]
                    if instrument:
                        if digest(instrument['path']) != instrument['sha256'] or any(
                                digest(path) != sha for path, sha in instrument['library_objects'].items()):
                            raise ValueError('Valgrind changed during measurement')
                        cmd = callgrind.command(instrument, engines[name], workload['path'], prefix)
                    if args.cpu is not None:
                        cmd = ['taskset', '-c', str(args.cpu), *cmd]
                    counters_path = prefix.with_suffix('.time-l.txt')
                    if args.darwin_counters:
                        cmd = ['/usr/bin/time', '-l', '-o', str(counters_path), *cmd]
                    perf_path = prefix.with_suffix('.perf-stat.txt')
                    if args.linux_counters:
                        cmd = ['perf', 'stat', '-x,', '-e', ','.join(LINUX_EVENTS), '-o', str(perf_path), '--', *cmd]
                    sample = run_sample(cmd, output, prefix, args.timeout)
                    semantic_status = admit(sample, workload['expected'].encode())
                    if semantic_status == 'ok' and Path(sample['stderr']).read_bytes():
                        semantic_status = 'unexpected-stderr'
                    status = semantic_status
                    if instrument:
                        profile_path = prefix.with_suffix('.callgrind')
                        log_path = prefix.with_suffix('.valgrind.log')
                        record = dict(raw_path=str(profile_path), log_path=str(log_path))
                        try:
                            record['raw_sha256'] = digest(profile_path)
                            record['log_sha256'] = digest(log_path)
                            record['values'] = callgrind.parse_profile(profile_path.read_text(),
                                                                      instrument['cache_config'])
                        except (OSError, ValueError) as error:
                            record['error'] = str(error)
                            if status == 'ok':
                                status = 'invalid-callgrind-counters'
                        sample['callgrind'] = record
                    if args.darwin_counters:
                        counter_record = {'scope': 'whole process including startup and teardown',
                                          'raw_path': str(counters_path)}
                        try:
                            counter_record['raw_sha256'] = digest(counters_path)
                            counter_record['values'] = parse_darwin_counters(counters_path.read_text())
                        except (OSError, ValueError) as error:
                            counter_record['error'] = str(error)
                            if status == 'ok':
                                status = 'invalid-hardware-counters'
                        sample['hardware_counters'] = counter_record
                    if args.linux_counters:
                        counter_record = {'scope': 'whole process including startup and teardown',
                                          'raw_path': str(perf_path)}
                        try:
                            counter_record['raw_sha256'] = digest(perf_path)
                            counter_record['values'] = parse_linux_counters(perf_path.read_text())
                        except (OSError, ValueError) as error:
                            counter_record['error'] = str(error)
                            if status == 'ok':
                                status = 'invalid-hardware-counters'
                        sample['hardware_counters'] = counter_record
                    sample.update(
                        case=workload['case'], size=workload['size'], engine=name,
                        repetition=repetition, semantic_status=semantic_status, status=status,
                    )
                    samples.append(sample)
                    journal.write(json.dumps(sample) + '\n')
                    journal.flush()
                    print(f'{prefix.name}: {status}', flush=True)
    summary = callgrind.summarize(samples) if instrument else summarize(samples)
    result = dict(metadata=metadata, summary=summary, samples=samples)
    (output / 'results.json').write_text(json.dumps(result, indent=2) + '\n')
    return 0 if all(row['eligible'] for row in summary) else 1
