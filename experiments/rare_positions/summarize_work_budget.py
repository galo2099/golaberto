#!/usr/bin/env python3
"""Summarize paired deterministic work-budget cohorts (JSON or JSON.gz)."""
import argparse
from collections import defaultdict
import gzip
import json
from pathlib import Path


def stats(rows):
    def total(arm, key):
        return sum(row['timing'][arm][key] for row in rows)
    bcpu, ccpu = total('baseline', 'cpu_ms'), total('candidate', 'cpu_ms')
    bwall, cwall = total('baseline', 'http_ms'), total('candidate', 'http_ms')
    gains = [(row['input'], row['seed'], cell) for row in rows
             for cell in row['comparison']['gains']]
    losses = [(row['input'], row['seed'], cell) for row in rows
              for cell in row['comparison']['lost']]
    def stage(arm, key):
        return sum(row['timing'][arm]['stages'].get(key, 0.) for row in rows)
    return dict(pairs=len(rows), baseline_cpu_ms=bcpu, candidate_cpu_ms=ccpu,
                cpu_change_percent=100. * (ccpu / bcpu - 1.),
                baseline_http_ms=bwall, candidate_http_ms=cwall,
                http_change_percent=100. * (cwall / bwall - 1.),
                baseline_tail_ms=stage('baseline', 'search.rare_tail'),
                candidate_tail_ms=stage('candidate', 'search.rare_tail'),
                gains=gains, losses=losses,
                distinct_gained_cells=sorted({(q, c['team'], c['position'])
                                             for q, _, c in gains}),
                proof_regressions=sum(len(row['comparison']['lost_reachability'])
                                      + len(row['comparison']['impossible_regressions'])
                                      for row in rows))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('cohorts', type=Path, nargs='+')
    parser.add_argument('--output', type=Path)
    args = parser.parse_args()
    rows = []
    for path in args.cohorts:
        opener = gzip.open if path.suffix == '.gz' else open
        with opener(path, 'rt') as stream:
            rows.extend(json.load(stream)['pairs'])
    grouped = defaultdict(list)
    for row in rows:
        grouped[row['input']].append(row)
    result = dict(aggregate=stats(rows),
                  groups={key: stats(value) for key, value in sorted(grouped.items())})
    text = json.dumps(result, indent=2) + '\n'
    if args.output:
        args.output.write_text(text)
    else:
        print(text, end='')


if __name__ == '__main__':
    main()
