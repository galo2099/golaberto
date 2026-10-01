//! Offline audit: inspect unresolved point-cap relaxations and exactly enumerate
//! small residual constraint components. Does not change production or the DB.
use golaberto_odds::{
    model::Model,
    proof::{max_gain, min_gain, Problem},
};
use serde_json::{json, Value};
use std::{collections::HashSet, fs};
#[derive(Default)]
struct Stats {
    nodes: usize,
    feasible: usize,
    components: usize,
    forests: usize,
    cycles: usize,
    small: usize,
    checked: usize,
    assignments: usize,
    contradictions: usize,
}
fn audit(p: &Problem, cap: i32, mask: u64, domains: &[u8], s: &mut Stats) {
    let n = p.base.len();
    let mut adjacency = vec![Vec::new(); n];
    let mut base = p.base.clone();
    for (i, g) in p.games.iter().enumerate() {
        let d = domains[i];
        if d.count_ones() == 1 {
            let o = d.trailing_zeros() as usize;
            base[g.home] += g.hg[o];
            base[g.away] += g.ag[o];
        } else {
            let h = mask & (1 << g.home) == 0;
            let a = mask & (1 << g.away) == 0;
            if h && a {
                adjacency[g.home].push((g.away, i));
                adjacency[g.away].push((g.home, i));
            } else if h {
                base[g.home] += min_gain(d, g.hg);
            } else if a {
                base[g.away] += min_gain(d, g.ag);
            }
        }
    }
    let mut seen = vec![false; n];
    for root in 0..n {
        if seen[root] || mask & (1 << root) != 0 {
            continue;
        }
        let mut vertices = vec![root];
        seen[root] = true;
        let mut pos = 0;
        while pos < vertices.len() {
            let t = vertices[pos];
            pos += 1;
            for &(v, _) in &adjacency[t] {
                if !seen[v] {
                    seen[v] = true;
                    vertices.push(v);
                }
            }
        }
        let mut edges = vertices
            .iter()
            .flat_map(|t| adjacency[*t].iter().map(|(_, i)| *i))
            .collect::<Vec<_>>();
        edges.sort_unstable();
        edges.dedup();
        if edges.is_empty() {
            continue;
        }
        s.components += 1;
        if edges.len() + 1 == vertices.len() {
            s.forests += 1;
        }
        if edges.len() == vertices.len() && vertices.iter().all(|t| adjacency[*t].len() == 2) {
            s.cycles += 1;
        }
        if edges.len() > 8 {
            continue;
        }
        s.small += 1;
        s.checked += 1;
        fn visit(
            p: &Problem,
            cap: i32,
            ds: &[u8],
            edges: &[usize],
            vs: &[usize],
            points: &mut [i32],
            step: usize,
            draws: &mut usize,
        ) -> bool {
            if step == edges.len() {
                *draws += 1;
                return vs.iter().all(|t| points[*t] <= cap);
            }
            let i = edges[step];
            let g = &p.games[i];
            for o in 0..3 {
                if ds[i] & (1 << o) == 0 {
                    continue;
                }
                points[g.home] += g.hg[o];
                points[g.away] += g.ag[o];
                let found = visit(p, cap, ds, edges, vs, points, step + 1, draws);
                points[g.home] -= g.hg[o];
                points[g.away] -= g.ag[o];
                if found {
                    return true;
                }
            }
            false
        }
        if !visit(
            p,
            cap,
            domains,
            &edges,
            &vertices,
            &mut base.clone(),
            0,
            &mut s.assignments,
        ) {
            s.contradictions += 1;
        }
    }
}
fn query(p: &Problem, t: usize, rank: usize, s: &mut Stats) {
    let cap = p.target_max(t);
    let mut ds = vec![7; p.games.len()];
    // Inspect both existing target-extreme patterns. Other games stay shared.
    for (i, g) in p.games.iter().enumerate() {
        let gains = if g.home == t {
            Some(g.hg)
        } else if g.away == t {
            Some(g.ag)
        } else {
            None
        };
        if let Some(gains) = gains {
            let best = max_gain(7, gains);
            ds[i] = (0..3)
                .filter(|o| gains[*o] == best)
                .fold(0, |d, o| d | (1 << o));
        }
    }
    let mut minimum = p.base.clone();
    for (g, d) in p.games.iter().zip(&ds) {
        minimum[g.home] += min_gain(*d, g.hg);
        minimum[g.away] += min_gain(*d, g.ag);
    }
    let mut mask = 1 << t;
    let mut mandatory = 0;
    for i in 0..p.base.len() {
        if !p.ranked[i] {
            mask |= 1 << i;
        } else if i != t && minimum[i] > cap {
            mask |= 1 << i;
            mandatory += 1;
        }
    }
    if mandatory > rank {
        return;
    }
    fn visit(
        p: &Problem,
        cap: i32,
        mask: u64,
        slots: usize,
        initial: &[u8],
        seen: &mut HashSet<u64>,
        nodes: &mut usize,
        s: &mut Stats,
    ) -> bool {
        if *nodes >= 500 || !seen.insert(mask) {
            return false;
        }
        *nodes += 1;
        s.nodes += 1;
        let mut ds = initial.to_vec();
        if p.propagate(cap, mask, &mut ds).is_some() {
            s.feasible += 1;
            audit(p, cap, mask, &ds, s);
            return true;
        }
        if slots == 0 {
            return false;
        }
        for i in 0..p.base.len() {
            if mask & (1 << i) != 0 || !p.ranked[i] {
                continue;
            }
            if visit(p, cap, mask | (1 << i), slots - 1, initial, seen, nodes, s) {
                return true;
            }
        }
        false
    }
    visit(
        p,
        cap,
        mask,
        rank - mandatory,
        &ds,
        &mut HashSet::new(),
        &mut 0,
        s,
    );
}
fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    assert_eq!(args.len(), 2, "REQUEST.json BASELINE-EXPORT.json");
    let m = Model::new(serde_json::from_slice(&fs::read(&args[0]).unwrap()).unwrap()).unwrap();
    let export: Value = serde_json::from_slice(&fs::read(&args[1]).unwrap()).unwrap();
    let Some(p) = Problem::new(&m) else {
        return;
    };
    let q = p.negated();
    let mut s = Stats::default();
    let mut cells = 0;
    for t in 0..m.n {
        for r in 0..m.n {
            let e = &export["rare_position_estimates"][m.ids[t].to_string()][r.to_string()];
            if e["probability"].as_f64() != Some(0.)
                || e["reachability"]
                    .as_str()
                    .unwrap_or("")
                    .starts_with("impossible")
            {
                continue;
            }
            cells += 1;
            query(&p, t, r, &mut s);
            query(&q, t, m.n - 1 - r, &mut s);
        }
    }
    println!(
        "{}",
        json!({"group":m.request.id,"zero_cells":cells,"query_nodes":s.nodes,"feasible_relaxations":s.feasible,
        "nontrivial_components":s.components,"forest_components":s.forests,"simple_cycles":s.cycles,
        "small_components_le8_edges":s.small,"enumerated_components":s.checked,"enumerated_assignments":s.assignments,
        "additional_fixed_exemption_contradictions":s.contradictions})
    );
}
