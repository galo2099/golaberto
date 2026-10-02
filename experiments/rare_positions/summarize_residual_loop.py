#!/usr/bin/env python3
"""Summarize cold paired Rust residual-loop benchmarks (JSON or JSON.gz)."""
import argparse
import gzip
import json
import statistics
from pathlib import Path


def summarize(rows):
    totals = {
        mode: {key: sum(row['timing'][mode][key] for row in rows)
               for key in ('cpu_ms', 'http_ms')}
        for mode in ('baseline', 'candidate')
    }
    gains = [(row['input'], cell['team'], cell['position'])
             for row in rows for cell in row['comparison']['gains']]
    events = [event for row in rows for event in row['timing']['candidate']['tail']]
    loops = [event for event in events
             if event['event'] == 'rust_odds_rare_tail_residual_loop']
    return {
        'pairs': len(rows),
        'total': totals,
        'change_percent': {
            key: 100 * (totals['candidate'][key] / totals['baseline'][key] - 1)
            for key in ('cpu_ms', 'http_ms')
        },
        'gained_cell_runs': len(gains),
        'distinct_gained_cells': len(set(gains)),
        'lost_cell_runs': sum(len(row['comparison']['lost']) for row in rows),
        'proof_regressions': sum(len(row['comparison']['impossible_regressions'])
                                 + len(row['comparison']['lost_reachability']) for row in rows),
        'reachable_zero_cell_runs': {
            mode: sum(row['timing'][mode]['cells']['reachable_zero'] for row in rows)
            for mode in ('baseline', 'candidate')
        },
        'mean_request_ms': {
            mode: statistics.mean(row['timing'][mode]['http_ms'] for row in rows)
            for mode in ('baseline', 'candidate')
        },
        'loop_accepted': [event for event in loops if event['accepted']],
        'loop_summaries': [event for event in events
                           if event['event'] == 'rust_odds_rare_tail_residual_loop_summary'],
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('summaries', type=Path, nargs='+')
    args = parser.parse_args()
    rows = []
    for path in args.summaries:
        opener = gzip.open if path.suffix == '.gz' else open
        with opener(path, 'rt') as handle:
            rows.extend(json.load(handle)['pairs'])
    result = summarize(rows)
    result['groups'] = {name: summarize([row for row in rows if row['input'] == name])
                        for name in sorted({row['input'] for row in rows})}
    print(json.dumps(result, indent=2))


if __name__ == '__main__':
    main()
