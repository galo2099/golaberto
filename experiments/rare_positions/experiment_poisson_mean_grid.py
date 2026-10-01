#!/usr/bin/env python3
"""Four-core paired HTTP sensitivity test for rounding actual Poisson means."""
import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import resource

from benchmark_joint_cap_requests import compare
from compare_rust_reachability import run_http


def rounded_request(request, step):
    data = json.loads(json.dumps(request))
    for game in data['games']:
        if game['played']:
            continue
        for key in ('home_power', 'away_power'):
            value = game[key]
            # Preserve structural zero/positive support, including values below half a bin.
            game[key] = max(step, math.floor(value / step + 0.5) * step) if value > 0 else 0
    return data


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--step', type=float, default=0.01)
    parser.add_argument('--seeds', default='1987,1993')
    args = parser.parse_args()
    if not math.isfinite(args.step) or args.step <= 0:
        parser.error('--step must be finite and positive')
    root = Path(__file__).resolve().parents[2]
    paths = sorted((root / 'experiments/rare_positions/reference/2026-09-30-hundredfold/inputs').glob('group-*.json'))
    paths.append(root / 'odds-rust/tests/fixtures/group-16653-2d1c1d6f.json')
    args.output.mkdir(parents=True, exist_ok=True)
    env = {k: v for k, v in os.environ.items() if not k.startswith(('RUST_ODDS_', 'RARE_POSITION_'))}
    summary = {'workers': 4, 'step': args.step, 'binary_sha256': hashlib.sha256(args.binary.read_bytes()).hexdigest(),
               'note': 'Actual target distributions differ; this is sensitivity, not a same-model accuracy comparison.', 'pairs': []}
    for path in paths:
        quantized = args.output / (path.stem + '-rounded-input.json')
        quantized.write_text(json.dumps(rounded_request(json.loads(path.read_text()), args.step)))
        for i, seed in enumerate(map(int, args.seeds.split(','))):
            timing, responses = {}, {}
            for arm in (['baseline', 'candidate'] if i % 2 == 0 else ['candidate', 'baseline']):
                result = args.output / f'{path.stem}-{seed}-{arm}.json'
                before = resource.getrusage(resource.RUSAGE_CHILDREN)
                wall, _, _, logs = run_http(args.binary.resolve(), path if arm == 'baseline' else quantized,
                                            seed, {}, result, env)
                after = resource.getrusage(resource.RUSAGE_CHILDREN)
                responses[arm] = json.loads(result.read_text())
                events = [json.loads(line) for line in logs.splitlines() if line.endswith('}')]
                timing[arm] = {'http_ms': wall, 'cpu_ms': 1000 * (after.ru_utime + after.ru_stime - before.ru_utime - before.ru_stime),
                               'stages': {e['stage']: e['elapsed_ms'] for e in events if e.get('event') == 'rust_odds_stage'}}
            row = {'input': path.name, 'seed': seed, 'original_sha256': hashlib.sha256(path.read_bytes()).hexdigest(),
                   'rounded_sha256': hashlib.sha256(quantized.read_bytes()).hexdigest(), 'timing': timing,
                   'comparison': compare(responses['baseline'], responses['candidate'])}
            summary['pairs'].append(row)
            print(json.dumps({'input': path.name, 'seed': seed, 'gained': len(row['comparison']['gains']),
                              'lost': row['comparison']['lost'], 'max_probability_difference': row['comparison']['max_existing_probability_difference']}), flush=True)
    (args.output / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')


if __name__ == '__main__':
    main()
