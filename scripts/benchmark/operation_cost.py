"""Reduce authenticated fixed.py Callgrind replays to marginal iteration costs."""
import argparse
from collections import defaultdict
import json
from pathlib import Path
import re
import statistics

from run import digest
from callgrind import parse_profile


def iteration_costs(report):
    metadata = report['metadata']
    instrument = metadata.get('callgrind')
    if not instrument:
        raise ValueError('operation costs require a Callgrind replay')
    workloads = {row['case']: row for row in metadata['workloads']}
    series = defaultdict(lambda: defaultdict(list))
    for sample in report['samples']:
        if sample['status'] != 'ok' or sample['semantic_status'] != 'ok':
            raise ValueError('cannot report costs from a failed workload')
        workload = workloads[sample['case']]
        match = re.fullmatch(r'(.+)_n([0-9]+)', sample['case'])
        if not match or int(match[2]) != workload['size'] or sample['size'] != workload['size']:
            raise ValueError('cost slopes require explicitly scaled workloads')
        profile = sample['callgrind']
        if digest(profile['raw_path']) != profile['raw_sha256']:
            raise ValueError('Callgrind evidence changed')
        values = parse_profile(Path(profile['raw_path']).read_text(), instrument['cache_config'])
        if values != profile['values']:
            raise ValueError('reported counts differ from raw evidence')
        series[sample['engine'], match[1]][workload['size']].append(values)
    results = []
    for (engine, case), points in sorted(series.items()):
        counts = sorted(points)
        if len(counts) < 3:
            raise ValueError('three iteration counts are required to check linearity')
        slopes = []
        for low, high in zip(counts, counts[1:]):
            costs = {event: (statistics.median(row[event] for row in points[high])
                             - statistics.median(row[event] for row in points[low])) / (high - low)
                     for event in ('Ir', 'Dr', 'Dw')}
            if costs['Ir'] <= 0:
                raise ValueError('instruction counts did not grow with work')
            slopes.append(dict(low=low, high=high, **costs))
        first = slopes[0]['Ir']
        spread = max(abs(row['Ir'] / first - 1) * 100 for row in slopes)
        results.append(dict(engine=engine, case=case, slopes=slopes,
                            instruction_linearity_difference_percent=spread))
    return dict(unit='native instructions or accesses per guest loop iteration; not per bytecode',
                results=results)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('report', type=Path)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    result = iteration_costs(json.loads(args.report.read_text()))
    result['source_report_sha256'] = digest(args.report)
    args.output.write_text(json.dumps(result, indent=2) + '\n')


if __name__ == '__main__':
    main()
