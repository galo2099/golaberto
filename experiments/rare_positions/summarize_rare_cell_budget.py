#!/usr/bin/env python3
"""Audit paired work increases against recovered zero-observation MC cells."""
import argparse
import json
from pathlib import Path
import statistics


def rare_cells(export):
    return {(int(t), int(r) + 1) for t, row in export['rare_position_estimates'].items()
            for r, e in row.items() if e['hits'] == 0 and e['probability'] > 0}


def summarize(directory):
    directory = Path(directory)
    source = json.loads((directory / 'summary.json').read_text())
    groups = {}
    for pair in source['pairs']:
        name, seed = pair['input'], pair['seed']
        exports = {side: json.loads((directory / f'{Path(name).stem}-{seed}-{side}.json').read_text())
                   for side in ('baseline', 'candidate')}
        a, b = (exports[side]['rare_position_estimates'] for side in ('baseline', 'candidate'))
        assert a.keys() == b.keys()
        assert all(a[t][r]['hits'] == b[t][r]['hits'] and
                   a[t][r]['samples'] == b[t][r]['samples'] for t in a for r in a[t]), \
            'Initial MC differs: this is not a paired rare-cell comparison'
        old, new = (rare_cells(exports[side]) for side in ('baseline', 'candidate'))
        row = {'seed': seed, 'baseline_rare': len(old), 'candidate_rare': len(new),
               'gains': sorted(new - old), 'losses': sorted(old - new),
               'http_ms': {s: pair['timing'][s]['http_ms'] for s in ('baseline', 'candidate')},
               'cpu_ms': {s: pair['timing'][s]['cpu_ms'] for s in ('baseline', 'candidate')}}
        groups.setdefault(name, []).append(row)
    out = {}
    for name, rows in groups.items():
        old = sum(r['baseline_rare'] for r in rows)
        new = sum(r['candidate_rare'] for r in rows)
        allowance = max(0., (new - old) / old) if old else 0.
        cpu = {s: sum(r['cpu_ms'][s] for r in rows) for s in ('baseline', 'candidate')}
        http = {s: sum(r['http_ms'][s] for r in rows) for s in ('baseline', 'candidate')}
        median = {s: statistics.median(r['http_ms'][s] for r in rows) for s in ('baseline', 'candidate')}
        out[name] = {'pairs': rows, 'baseline_rare_cell_runs': old, 'candidate_rare_cell_runs': new,
                     'gain_cell_runs': sum(len(r['gains']) for r in rows),
                     'loss_cell_runs': sum(len(r['losses']) for r in rows),
                     'distinct_gained_cells': sorted({tuple(c) for r in rows for c in r['gains']}),
                     'distinct_lost_cells': sorted({tuple(c) for r in rows for c in r['losses']}),
                     'permitted_work_increase_pct': 100 * allowance, 'cpu_ms': cpu, 'http_ms': http,
                     'median_http_ms': median,
                     'cpu_change_pct': 100 * (cpu['candidate'] / cpu['baseline'] - 1),
                     'total_http_change_pct': 100 * (http['candidate'] / http['baseline'] - 1),
                     'median_http_change_pct': 100 * (median['candidate'] / median['baseline'] - 1),
                     'worst_paired_http_change_pct': max(100 * (r['http_ms']['candidate'] / r['http_ms']['baseline'] - 1) for r in rows)}
    return {'rare_definition': 'probability > 0 and initial 100k MC hits == 0',
            'allowance_rule': 'max(0, net recovered rare cells / baseline recovered rare cells)',
            'note': 'Configured operation limits, reserved operation units, CPU, and wall time are distinct measures.',
            'groups': out}


if __name__ == '__main__':
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('directory', type=Path)
    p.add_argument('--output', type=Path)
    a = p.parse_args()
    encoded = json.dumps(summarize(a.directory), indent=2) + '\n'
    if a.output:
        a.output.write_text(encoded)
    else:
        print(encoded, end='')
