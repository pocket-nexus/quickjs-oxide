"""Analyze complete balanced fixed.py blocks, using processes as observations.

The reported t interval is descriptive: consecutive blocks can still share
host drift. A/A exposes that drift and must be read alongside A/B.
"""
import argparse
import json
import math
from pathlib import Path
import statistics


T_975 = (
    0, 12.706, 4.303, 3.182, 2.776, 2.571, 2.447, 2.365, 2.306,
    2.262, 2.228, 2.201, 2.179, 2.160, 2.145, 2.131, 2.120, 2.110,
    2.101, 2.093, 2.086, 2.080, 2.074, 2.069, 2.064, 2.060, 2.056,
    2.052, 2.048, 2.045, 2.042,
)


def blocks(result, left, right):
    metadata = result['metadata']
    if metadata['order_strategy'] not in ('abba', 'baab', 'abba-baab'):
        raise ValueError('balanced two-engine order required')
    repeat = metadata['repeat']
    if repeat < 4 or repeat % 2:
        raise ValueError('at least two complete two-repetition blocks required')
    if set(metadata['engines']) != {left, right}:
        raise ValueError('engine names do not match result')
    by_case = {}
    for sample in result['samples']:
        if sample['status'] != 'ok' or sample['semantic_status'] != 'ok':
            raise ValueError('incomplete or invalid process sample')
        key = (sample['case'], sample['repetition'], sample['engine'])
        if key in by_case:
            raise ValueError('duplicate process sample')
        if sample['process_wall_ns'] <= 0:
            raise ValueError('invalid process wall time')
        by_case[key] = sample['process_wall_ns']
    cases = {sample['case'] for sample in result['samples']}
    expected = {(case, repetition, engine)
                for case in cases for repetition in range(repeat)
                for engine in (left, right)}
    if set(by_case) != expected:
        raise ValueError('missing or unexpected process sample')
    return {
        case: [
            statistics.mean(
                math.log(by_case[case, repetition, right] /
                         by_case[case, repetition, left])
                for repetition in (start, start + 1)
            )
            for start in range(0, repeat, 2)
        ]
        for case in sorted(cases)
    }


def describe(log_ratios):
    count = len(log_ratios)
    if count < 2:
        raise ValueError('at least two blocks required')
    mean = statistics.mean(log_ratios)
    half_width = T_975[min(count - 1, 30)] * statistics.stdev(log_ratios) / math.sqrt(count)
    return {
        'blocks': count,
        'block_log_ratios': log_ratios,
        'geometric_ratio': math.exp(mean),
        'descriptive_95pct_interval': [math.exp(mean - half_width), math.exp(mean + half_width)],
    }


def analyze(ab, aa, reference, candidate, aa_left, aa_right):
    ab_meta, aa_meta = ab['metadata'], aa['metadata']
    if ab_meta['manifest_sha256'] != aa_meta['manifest_sha256']:
        raise ValueError('A/B and A/A used different workload manifests')
    if ab_meta['repeat'] != aa_meta['repeat'] or ab_meta['order_strategy'] != aa_meta['order_strategy']:
        raise ValueError('A/B and A/A have different schedules')
    candidate_hash = ab_meta['engines'][candidate]['sha256']
    if any(aa_meta['engines'][name]['sha256'] != candidate_hash for name in (aa_left, aa_right)):
        raise ValueError('A/A binaries do not match candidate')
    ab_blocks = blocks(ab, reference, candidate)
    aa_blocks = blocks(aa, aa_left, aa_right)
    if ab_blocks.keys() != aa_blocks.keys():
        raise ValueError('A/B and A/A have different cases')
    return {
        'method': 'two repetitions per balanced block; mean log(candidate/reference) and descriptive t interval over blocks',
        'caveat': 'host drift can correlate adjacent blocks; A/A is a drift control, not a formal confidence interval',
        'manifest_sha256': ab_meta['manifest_sha256'],
        'reference_sha256': ab_meta['engines'][reference]['sha256'],
        'candidate_sha256': candidate_hash,
        'cases': {case: {'ab': describe(ab_blocks[case]), 'aa': describe(aa_blocks[case])}
                  for case in ab_blocks},
    }


def markdown(report):
    rows = [
        '# Balanced fixed-work blocks', '',
        'Ratios below 1 favor the candidate. Intervals use complete two-repetition process blocks, not in-process iterations.',
        '',
        '| Case | A/B ratio | Descriptive 95% interval | A/A ratio | Blocks |',
        '| --- | ---: | ---: | ---: | ---: |',
    ]
    for case, values in report['cases'].items():
        ab, aa = values['ab'], values['aa']
        low, high = ab['descriptive_95pct_interval']
        rows.append(f"| {case} | {ab['geometric_ratio']:.4f}× | [{low:.4f}, {high:.4f}] | "
                    f"{aa['geometric_ratio']:.4f}× | {ab['blocks']} |")
    rows.extend(['', report['caveat'], ''])
    return '\n'.join(rows)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--ab', type=Path, required=True)
    parser.add_argument('--aa', type=Path, required=True)
    parser.add_argument('--reference', required=True)
    parser.add_argument('--candidate', required=True)
    parser.add_argument('--aa-left', default='left')
    parser.add_argument('--aa-right', default='right')
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    report = analyze(json.loads(args.ab.read_text()), json.loads(args.aa.read_text()),
                     args.reference, args.candidate, args.aa_left, args.aa_right)
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    (output / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
    (output / 'report.md').write_text(markdown(report))
    print(output / 'report.md')


if __name__ == '__main__':
    main()
