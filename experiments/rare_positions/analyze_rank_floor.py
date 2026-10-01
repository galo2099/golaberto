#!/usr/bin/env python3
"""Independent offline points/wins worst-rank checker (no probability estimate).

All W/D/L outcomes are allowed. Final P/W equality counts as ahead, so only
exhaustive infeasibility proves impossibility. A budget exhaustion is unknown.
Uses only endpoint propagation, independently of the Rust aggregate cuts.
"""
import argparse, itertools, json, time
from pathlib import Path
ap = argparse.ArgumentParser()
ap.add_argument('request')
ap.add_argument('team', type=int)
ap.add_argument('rank', type=int)
ap.add_argument('--nodes', type=int, default=200000)
ap.add_argument('--output', required=True)
args = ap.parse_args()
r = json.loads(Path(args.request).read_text())
phase = r['phase']
rules = phase['championship']
if phase['sort'].split(',')[:2] != ['pt', 'w'] or phase.get('bonus_points', 0) or tuple((rules[k] for k in ['point_win', 'point_draw', 'point_loss'])) != (3, 1, 0):
    raise ValueError('requires points then wins, standard 3/1/0 scoring, no bonus points')
if not 1 <= args.rank <= len(r['team_groups']) or args.nodes < 0:
    raise ValueError('invalid rank or node budget')
if any((t not in {x['team_id'] for x in r['team_groups']} for g in r['games'] for t in [g['home_id'], g['away_id']])):
    raise ValueError('independent checker requires all fixture teams in the group')
ids = [t['team_id'] for t in r['team_groups']]
index = {t: i for (i, t) in enumerate(ids)}
n = len(ids)
target = index[args.team]
base = [t.get('add_sub', 0) for t in r['team_groups']]
wins = [0] * n
left = [0] * n
raw = []
for g in r['games']:
    (h, a) = (index[g['home_id']], index[g['away_id']])
    if g['played']:
        (x, y) = (g['home_score'], g['away_score'])
        base[h] += 3 * (x > y) + (x == y)
        base[a] += 3 * (x < y) + (x == y)
        wins[h] += x > y
        wins[a] += x < y
    else:
        raw.append((h, a, g['id']))
        left[h] += 1
        left[a] += 1
stride = max((w + l for (w, l) in zip(wins, left))) + 1
initial = [p * stride + w for (p, w) in zip(base, wins)]
floor = initial[target]
hg = (0, stride, 3 * stride + 1)
ag = tuple(reversed(hg))
m = len(raw)
# Target losses give a necessary worst-rank relaxation.
domains = [1 if h == target else 4 if a == target else 7 for (h, a, _) in raw]
maxima = initial.copy()
for ((h, a, _), d) in zip(raw, domains):
    maxima[h] += max((hg[o] for o in range(3) if d & 1 << o))
    maxima[a] += max((ag[o] for o in range(3) if d & 1 << o))
eligible = [t for t in range(n) if t != target and maxima[t] >= floor]
nodes = 0
records = []
found = None
start = time.monotonic()
opts = {d: [o for o in range(3) if d & 1 << o] for d in range(1, 8)}
# Maximum endpoint gains for every nonempty domain.
hmax = [0] + [max((hg[o] for o in opts[d])) for d in range(1, 8)]
amax = [0] + [max((ag[o] for o in opts[d])) for d in range(1, 8)]

def propagate(ds, required):
    while True:
        mx = initial.copy()
        for (i, (h, a, _)) in enumerate(raw):
            d = ds[i]
            if not d:
                return None
            mx[h] += hmax[d]
            mx[a] += amax[d]
        if any((mx[t] < floor for t in required)):
            return None
        changed = False
        for (i, (h, a, _)) in enumerate(raw):
            d = ds[i]
            nd = d
            for o in opts[d]:
                if h in required and mx[h] - hmax[d] + hg[o] < floor or (a in required and mx[a] - amax[d] + ag[o] < floor):
                    nd &= ~(1 << o)
            if not nd:
                return None
            if nd != d:
                ds[i] = nd
                changed = True
        if not changed:
            return mx

def dfs(ds, required):
    global nodes
    if nodes >= args.nodes:
        return ('budget', None)
    nodes += 1
    mx = propagate(ds, required)
    if mx is None:
        return ('impossible', None)
    choices = [i for (i, d) in enumerate(ds) if len(opts[d]) > 1]
    if not choices:
        return ('feasible', [opts[d][0] for d in ds])

    def priority(i):
        (h, a, _) = raw[i]
        slack = min([mx[t] - floor for t in (h, a) if t in required] or [10 ** 9])
        return (len(opts[ds[i]]), slack, i)
    i = min(choices, key=priority)
    (h, a, _) = raw[i]
    choices = sorted(opts[ds[i]], key=lambda o: -(hg[o] * (h in required) + ag[o] * (a in required)))
    for o in choices:
        nxt = ds.copy()
        nxt[i] = 1 << o
        (status, witness) = dfs(nxt, required)
        if status != 'impossible':
            return (status, witness)
    return ('impossible', None)
for cohort in itertools.combinations(eligible, args.rank - 1):
    before = nodes
    (status, witness) = dfs(domains.copy(), set(cohort))
    records.append(dict(required=[ids[t] for t in cohort], status=status, nodes=nodes - before))
    if status == 'feasible':
        found = witness
        break
    if status == 'budget':
        break
result = dict(group=r['id'], target=args.team, rank=args.rank, target_points=base[target], target_wins=wins[target], stride=stride, eligible=[ids[t] for t in eligible], cohorts_total=len(list(itertools.combinations(eligible, args.rank - 1))), cohorts_tested=len(records), nodes=nodes, elapsed_ms=(time.monotonic() - start) * 1000, records=records)
if found is not None:
    p = base.copy()
    w = wins.copy()
    for ((h, a, _), o) in zip(raw, found):
        p[h] += (0, 1, 3)[o]
        p[a] += (3, 1, 0)[o]
        w[h] += o == 2
        w[a] += o == 0
    result['assignment'] = [dict(game_id=g, outcome=o) for ((_, _, g), o) in zip(raw, found)]
    result['final_points_wins'] = [dict(team=t, points=p[i], wins=w[i]) for (i, t) in enumerate(ids)]
    result['strict_ahead_points_wins'] = sum(((p[i], w[i]) > (p[target], w[target]) for i in range(n) if i != target))
    result['equal_points_wins'] = [ids[i] for i in range(n) if i != target and (p[i], w[i]) == (p[target], w[target])]
else:
    result['impossible_points_wins'] = len(records) == result['cohorts_total'] and all((x['status'] == 'impossible' for x in records))
Path(args.output).write_text(json.dumps(result, indent=2) + '\n')
print(json.dumps({k: v for (k, v) in result.items() if k not in ['records', 'assignment', 'final_points_wins']}, indent=2), flush=True)
