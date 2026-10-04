"""Callgrind evidence for the existing authenticated fixed-work replayer."""
import json
import os
import re
import statistics
from pathlib import Path

from run import command_output, digest


REQUIRED_EVENTS = ('Ir', 'Dr', 'Dw', 'I1mr', 'D1mr', 'D1mw', 'Bc', 'Bcm', 'Bi', 'Bim')
CACHE_NAMES = ('I1', 'D1', 'LL')


def load_cache_config(path):
    config = json.loads(Path(path).read_text())
    if not isinstance(config, dict) or set(config) != set(CACHE_NAMES):
        raise ValueError('cache config must contain exactly I1, D1 and LL')
    for name, values in config.items():
        if (not isinstance(values, list) or len(values) != 3
                or any(type(value) is not int or value <= 0 for value in values)):
            raise ValueError(f'{name} must be [bytes, associativity, line_bytes]')
        size, ways, line = values
        sets, remainder = divmod(size, ways * line)
        if remainder or not sets or sets & (sets - 1) or line & (line - 1):
            raise ValueError(f'invalid {name} cache geometry')
    return config


def tool_metadata(binary, config_path, instruction_positions=False):
    binary = Path(binary).resolve()
    if not os.access(binary, os.X_OK):
        raise ValueError('Callgrind requires an executable Valgrind path')
    version = command_output([str(binary), '--version'])
    if version['exit_code'] or not version['stdout'].startswith('valgrind-'):
        raise ValueError('Valgrind version command failed')
    library = os.environ.get('VALGRIND_LIB')
    objects = {}
    if library:
        for name in ('callgrind-amd64-linux', 'vgpreload_core-amd64-linux.so'):
            path = Path(library) / name
            if not path.is_file():
                raise ValueError(f'missing Valgrind component: {path}')
            objects[str(path.resolve())] = digest(path)
    return dict(path=str(binary), sha256=digest(binary), version=version,
                runner_sha256=digest(__file__),
                library_objects=objects, cache_config=load_cache_config(config_path),
                cache_config_sha256=digest(config_path), branch_simulation=True,
                instruction_positions=instruction_positions,
                counter_scope='whole process including startup and teardown',
                cache_scope='simulated caches; not hardware event counters')


def command(metadata, engine, workload, prefix):
    options = [metadata['path'], '--tool=callgrind', '--cache-sim=yes', '--branch-sim=yes',
               '--callgrind-out-file=' + str(prefix.with_suffix('.callgrind')),
               '--log-file=' + str(prefix.with_suffix('.valgrind.log'))]
    options += [f'--{name}=' + ','.join(map(str, metadata['cache_config'][name]))
                for name in CACHE_NAMES]
    if metadata['instruction_positions']:
        options += ['--dump-instr=yes', '--collect-jumps=yes']
    return [*options, str(engine), workload]


def parse_profile(raw, expected_caches):
    """Use the final totals; Callgrind's header summary can include extra events."""
    def field(name):
        values = re.findall(r'^' + re.escape(name) + r':\s*(.*?)\s*$', raw, re.M)
        if len(values) != 1:
            raise ValueError(f'missing or duplicate Callgrind {name}')
        return values[0]

    if not raw.startswith('# callgrind format\n') or field('version') != '1':
        raise ValueError('unsupported Callgrind format')
    events = field('events').split()
    if len(events) != len(set(events)) or not set(REQUIRED_EVENTS).issubset(events):
        raise ValueError('missing or duplicate Callgrind events')
    words = field('totals').split()
    if len(words) != len(events) or any(not re.fullmatch(r'\d+', word) for word in words):
        raise ValueError('invalid Callgrind totals')
    values = dict(zip(events, map(int, words)))
    if values['Ir'] <= 0:
        raise ValueError('empty Callgrind instruction count')
    for name, expected in expected_caches.items():
        pattern = (r'^desc: ' + name + r' cache: (\d+) B, (\d+) B, '
                   r'(direct-mapped|(\d+)-way associative)\s*$')
        matches = list(re.finditer(pattern, raw, re.M))
        if len(matches) != 1:
            raise ValueError(f'missing or duplicate {name} cache description')
        match = matches[0]
        actual = [int(match[1]), int(match[4]) if match[4] else 1, int(match[2])]
        if actual != expected:
            raise ValueError(f'{name} cache differs from frozen model')
    return values


def summarize(samples):
    """Do not publish instrumented process times as a native speed result."""
    groups = {}
    for sample in samples:
        groups.setdefault((sample['case'], sample['engine']), []).append(sample)
    rows = []
    for (case, engine), group in groups.items():
        eligible = all(sample['status'] == 'ok' for sample in group)
        rows.append(dict(case=case, engine=engine, samples=len(group), eligible=eligible,
                         median_counters={event: statistics.median(
                             sample['callgrind']['values'][event] for sample in group)
                             for event in REQUIRED_EVENTS} if eligible else None))
    return rows
