"""Replay a recorded fixed-work matrix, verifying bytes before measuring."""
import argparse
import json
import os
import platform
import re
from pathlib import Path
from run import binary_metadata, digest, machine_metadata, paired_order, run_sample, validate_paired_order
from scaling import admit, summarize


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


def main():
    parser = argparse.ArgumentParser(description=__doc__)
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
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if args.repeat < 1 or args.timeout <= 0 or (args.cpu is not None and args.cpu < 0):
        parser.error('invalid repeat, timeout or CPU')
    if args.darwin_counters and (platform.system() != 'Darwin' or not os.access('/usr/bin/time', os.X_OK)):
        parser.error('--darwin-counters requires macOS /usr/bin/time')
    if args.darwin_counters and args.cpu is not None:
        parser.error('--cpu uses taskset and cannot be combined with --darwin-counters')
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
        counter_scope='whole process including startup and teardown' if args.darwin_counters else None,
        metric='whole-process wall nanoseconds; not adaptive scores',
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
                    if args.cpu is not None:
                        cmd = ['taskset', '-c', str(args.cpu), *cmd]
                    counters_path = prefix.with_suffix('.time-l.txt')
                    if args.darwin_counters:
                        cmd = ['/usr/bin/time', '-l', '-o', str(counters_path), *cmd]
                    sample = run_sample(cmd, output, prefix, args.timeout)
                    semantic_status = admit(sample, workload['expected'].encode())
                    if semantic_status == 'ok' and Path(sample['stderr']).read_bytes():
                        semantic_status = 'unexpected-stderr'
                    status = semantic_status
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
                    sample.update(
                        case=workload['case'], size=workload['size'], engine=name,
                        repetition=repetition, semantic_status=semantic_status, status=status,
                    )
                    samples.append(sample)
                    journal.write(json.dumps(sample) + '\n')
                    journal.flush()
                    print(f'{prefix.name}: {status}', flush=True)
    summary = summarize(samples)
    result = dict(metadata=metadata, summary=summary, samples=samples)
    (output / 'results.json').write_text(json.dumps(result, indent=2) + '\n')
    return 0 if all(row['eligible'] for row in summary) else 1


if __name__ == '__main__':
    raise SystemExit(main())
