#!/usr/bin/env python3
"""Build isolated paired Rust revisions and benchmark seven snapshots on four cores."""
import argparse
import gzip
import json
import os
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[2]
BASELINE = 'e9cce7dcf570ba53b2d489add5569db9eedd45db'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--baseline-revision', default=BASELINE)
    parser.add_argument('--candidate-patch', type=Path, required=True)
    parser.add_argument('--pgo', action='store_true')
    parser.add_argument('--persistent', action='store_true')
    parser.add_argument('--seeds', default='1601,1607,1613')
    args = parser.parse_args()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    work = Path(tempfile.mkdtemp(prefix='sources-', dir=output))
    archive = subprocess.check_output(['git', 'archive', args.baseline_revision,
        'odds-rust', 'stats/core',
        'experiments/rare_positions/reference/2026-09-30-hundredfold/inputs'], cwd=ROOT)
    patch = (gzip.open(args.candidate_patch, 'rb').read()
             if args.candidate_patch.suffix == '.gz' else args.candidate_patch.read_bytes())
    env = {k: v for k, v in os.environ.items() if k not in
           ('RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS', 'CARGO_TARGET_DIR')}
    binaries = {}
    for arm in ('baseline', 'candidate'):
        source = work / arm
        source.mkdir()
        subprocess.run(['tar', '-x', '-C', str(source)], input=archive, check=True)
        if arm == 'candidate':
            subprocess.run(['git', 'apply', '--check', '-'], cwd=source, input=patch, check=True)
            subprocess.run(['git', 'apply', '-'], cwd=source, input=patch, check=True)
        target = work / (arm + '-target')
        subprocess.run(['cargo', 'build', '--release', '--locked', '-j', '4',
                        '--manifest-path', str(source / 'odds-rust/Cargo.toml')],
                       env=dict(env, CARGO_TARGET_DIR=str(target)), check=True)
        binaries[arm] = target / 'release/golaberto-odds'
        if arm == 'candidate' and args.pgo:
            log = output / 'pgo-build.log'
            with log.open('w') as dst:
                subprocess.run(['python3', str(ROOT / 'experiments/rare_positions/build_rust_pgo.py'),
                                '--repo', str(source), '--output', str(output / 'pgo')],
                               env=env, stdout=dst, stderr=subprocess.STDOUT, check=True)
            binaries[arm] = Path(log.read_text().splitlines()[-1])
    metadata = {'compiler': subprocess.check_output(['rustc', '-Vv'], text=True),
                'uname': subprocess.check_output(['uname', '-a'], text=True),
                'baseline_revision': args.baseline_revision, 'workers': 4,
                'pgo': args.pgo, 'patch': str(args.candidate_patch.resolve())}
    if Path('/proc/cpuinfo').exists():
        metadata['cpuinfo'] = Path('/proc/cpuinfo').read_text()
    (output / 'host.json').write_text(json.dumps(metadata, indent=2) + '\n')
    command = ['python3', str(ROOT / 'experiments/rare_positions/benchmark_propagated_joint.py'),
               '--baseline', str(binaries['baseline']), '--candidate', str(binaries['candidate']),
               '--production-candidate', '--seeds', args.seeds, '--output', str(output / 'paired')]
    if args.persistent:
        command.append('--persistent')
    subprocess.run(command, env=env, check=True)
    rows = json.loads((output / 'paired/summary.json').read_text())['pairs']
    if args.persistent:
        cpu = {arm: sum(row['resources'][arm]['cpu_seconds'] for row in rows)
               for arm in binaries}
        comparisons = [c for row in rows for c in row['comparisons']]
    else:
        cpu = {arm: sum(row['timing'][arm]['cpu_ms'] for row in rows) for arm in binaries}
        comparisons = [row['comparison'] for row in rows]
    print(json.dumps({'cpu_change_percent': 100 * (cpu['candidate'] / cpu['baseline'] - 1),
                      'gained_cell_runs': sum(len(c['gains']) for c in comparisons),
                      'lost_cell_runs': sum(len(c['lost']) for c in comparisons)}, indent=2))


if __name__ == '__main__':
    main()
