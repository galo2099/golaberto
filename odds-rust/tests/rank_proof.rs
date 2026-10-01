use golaberto_odds::{
    model::{Model, Request},
    rank_proof::{Options, RankProof},
    search::Cell,
};
use serde_json::json;

#[test]
fn fixture_conflicts_match_exhaustive_seasons_on_varied_leagues() {
    let edges = [(0, 1), (1, 2), (2, 3), (3, 4), (4, 0), (0, 2)];
    let mut seed = 149_u64;
    for case in 0..20 {
        let mut next = || {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            seed >> 32
        };
        let teams: Vec<_> = (0..5)
            .map(|t| json!({"team_id":t,"add_sub":next() as i32 % 7}))
            .collect();
        let mut games:Vec<_>=edges.iter().enumerate().map(|(i,&(h,a))|json!({"id":i,"home_id":h,"away_id":a,"home_power":1.0,"away_power":1.0})).collect();
        for i in 0..2 {
            games.push(json!({"id":6+i,"home_id":i,"away_id":i+2,"played":true,"home_score":next()%3,"away_score":next()%3}));
        }
        let request:Request=serde_json::from_value(json!({"id":case,"phase":{"sort":"pt,w,gd,gf","championship":{"point_win":3,"point_draw":1,"point_loss":0}},"team_groups":teams,"games":games})).unwrap();
        let m = Model::new(request).unwrap();
        let p = RankProof::new(&m).unwrap();
        let negative = p.negated();
        let stride = (0..m.n)
            .map(|t| {
                m.base[t].wins
                    + m.fixtures
                        .iter()
                        .filter(|g| g.home == t || g.away == t)
                        .count() as i32
            })
            .max()
            .unwrap()
            + 1;
        let mut max_ahead = [[0; 5]; 2];
        for mut code in 0..3_usize.pow(m.fixtures.len() as u32) {
            let mut points: Vec<_> = m.base.iter().map(|c| c.points).collect();
            let mut wins: Vec<_> = m.base.iter().map(|c| c.wins).collect();
            for g in &m.fixtures {
                let o = code % 3;
                code /= 3;
                points[g.home] += [0, 1, 3][o];
                points[g.away] += [3, 1, 0][o];
                wins[g.home] += i32::from(o == 2);
                wins[g.away] += i32::from(o == 0);
            }
            let totals: Vec<_> = points
                .iter()
                .zip(wins)
                .map(|(pt, w)| pt * stride + w)
                .collect();
            for t in 0..5 {
                max_ahead[0][t] = max_ahead[0][t].max(
                    (0..5)
                        .filter(|o| *o != t && totals[*o] >= totals[t])
                        .count(),
                );
                max_ahead[1][t] = max_ahead[1][t].max(
                    (0..5)
                        .filter(|o| *o != t && totals[*o] <= totals[t])
                        .count(),
                );
            }
        }
        for (direction, proof) in [&p, &negative].into_iter().enumerate() {
            for team in 0..5 {
                for required in 0..5 {
                    for options in [
                        Options {
                            explain: false,
                            internal_order: false,
                            reuse_cohorts: false,
                            require_root_pressure: false,
                        },
                        Options::default(),
                    ] {
                        let r = proof.prove(
                            Cell {
                                team,
                                rank: required,
                            },
                            required,
                            100000,
                            options,
                        );
                        assert_eq!(
                            r.impossible,
                            max_ahead[direction][team] < required,
                            "case={case} direction={direction} team={team} required={required}"
                        );
                    }
                    let gated = proof.prove(
                        Cell {
                            team,
                            rank: required,
                        },
                        required,
                        500,
                        Options {
                            require_root_pressure: true,
                            ..Options::default()
                        },
                    );
                    if gated.impossible {
                        assert!(max_ahead[direction][team] < required);
                    }
                }
            }
        }
    }
}
