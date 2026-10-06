#!/usr/bin/env python3
"""Build an isolated replacement experiment; leaves production sources untouched."""
import argparse,shutil,subprocess,json,hashlib
from pathlib import Path
HOOK=r'''
    if std::env::var("RUST_ODDS_JOINT_CAP_EXPERIMENT").as_deref() == Ok("replace") {
        let joint = parallel(cells.len(), workers, |i| {
            let cell = cells[i];
            let search = &results[i];
            if search.impossible || search.result.samples == 0 || search.result.hits > 0 {
                return None;
            }
            let start = std::time::Instant::now();
            let plan = [false, true].into_iter().find_map(|dual| {
                [6, 5].into_iter().find_map(|count| crate::joint_caps::JointCaps::new(model, cell, dual, count))
            })?;
            let setup_ms = start.elapsed().as_secs_f64() * 1000.;
            let mut result = plan.sample(model, 5000, derive(seed, &format!("joint-cap-final-{}-{}", model.ids[cell.team], cell.rank)), true);
            let check = plan.sample(model, 2000, derive(seed, &format!("joint-cap-check-{}-{}", model.ids[cell.team], cell.rank)), true);
            let valid = result.coarse() && check.hits >= 30 && check.ess >= 5.
                && check.probability > 0. && check.std_err / check.probability <= 0.6
                && (0.1..=10.).contains(&(result.probability / check.probability));
            result.work += check.work;
            if std::env::var("RUST_ODDS_LOG").as_deref() != Ok("0") {
                eprintln!("{}", serde_json::json!({"event":"rust_odds_joint_cap_experiment", "group":model.request.id,
                    "setup":plan.describe(model), "setup_ms":setup_ms,"elapsed_ms":start.elapsed().as_secs_f64()*1000.,
                    "probability":result.probability,"hits":result.hits,"ess":result.ess,"relative_se":if result.probability>0.{Some(result.std_err/result.probability)}else{None},
                    "check_probability":check.probability,"check_hits":check.hits,"check_ess":check.ess,"accepted":valid}));
            }
            Some((result, valid))
        });
        for (i, result) in joint.into_iter().enumerate() {
            if let Some((mut result, valid)) = result {
                if valid {
                    result.work += results[i].result.work;
                    results[i].result = result;
                    results[i].event = None;
                    // Keep new sampler seasons out of the existing neighborhood
                    // seed allocation; the accepted estimate carries its witness.
                } else {
                    results[i].result.work += result.work;
                }
            }
        }
    }
'''
def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--allocation',choices=['early','deferred'],default='deferred');p.add_argument('--directory',type=Path,required=True);p.add_argument('--target',type=Path,required=True);a=p.parse_args()
 root=Path(__file__).resolve().parents[2];d=a.directory.resolve();d.mkdir(parents=True,exist_ok=True)
 shutil.copytree(root/'odds-rust',d/'odds-rust',ignore=shutil.ignore_patterns('target'),dirs_exist_ok=True)
 shutil.copytree(root/'experiments/rare_positions/reference',d/'experiments/rare_positions/reference',dirs_exist_ok=True)
 # Reconstruct the recorded pre-integration source, so this historical
 # experiment remains reproducible after production starts exporting the module.
 snapshot=json.loads((root/'experiments/rare_positions/results/2026-09-30-rust-joint-caps.json').read_text())['baseline_source_snapshot']
 for rel in snapshot['source_hashes']:
  (d/rel).write_bytes(subprocess.check_output(['git','show',snapshot['base_commit']+':'+rel],cwd=root))
 subprocess.run(['patch','-t','-p1','-i',str(root/snapshot['tracked_source_diff'])],cwd=d,check=True,stdout=subprocess.DEVNULL)
 for rel,sha in snapshot['source_hashes'].items():
  if hashlib.sha256((d/rel).read_bytes()).hexdigest()!=sha:raise SystemExit('Baseline source checksum mismatch: '+rel)
 p=d/'odds-rust/src/joint_caps.rs';p.write_text(p.read_text().replace('use golaberto_odds::','use crate::'))
 p=d/'odds-rust/src/lib.rs';p.write_text(p.read_text()+'\npub mod joint_caps;\n')
 p=d/'odds-rust/src/search.rs';s=p.read_text()
 if a.allocation=='early':
  marker='    let plans = parallel(cells.len(), workers, |i| {';assert s.count(marker)==1;s=s.replace(marker,HOOK+marker)
 else:
  start=s.index('fn extra(');end=s.index('pub fn apply(',start);part=s[start:end]
  part=part.replace('    results: &mut [SearchResult],\n) {','    results: &mut [SearchResult],\n) -> Vec<(Cell, Result)> {',1)
  marker='        let seed = derive(\n            seed,\n            &format!(\n                "conditioned-zero-extra-{}-{}",'
  pos=part.index(marker)
  native=r'''        let mut native = None;
        let mut joint_work = 0;
        let mut ordinary_draws = 50000;
        if std::env::var("RUST_ODDS_JOINT_CAP_EXPERIMENT").as_deref() == Ok("replace") {
            let start = std::time::Instant::now();
            if let Some(plan) = [false, true].into_iter().find_map(|dual| [6, 5].into_iter().find_map(|count| crate::joint_caps::JointCaps::new(model, cell, dual, count))) {
                let setup_ms = start.elapsed().as_secs_f64()*1000.;
                let mut result = plan.sample(model, 5000, derive(seed, &format!("joint-cap-final-{}-{}", model.ids[cell.team], cell.rank)), true);
                let check = plan.sample(model, 2000, derive(seed, &format!("joint-cap-check-{}-{}", model.ids[cell.team], cell.rank)), true);
                let valid = result.coarse() && check.hits >= 30 && check.ess >= 5.
                    && check.probability > 0. && check.std_err / check.probability <= 0.6
                    && (0.1..=10.).contains(&(result.probability/check.probability));
                joint_work = result.work + check.work;
                if std::env::var("RUST_ODDS_LOG").as_deref() != Ok("0") {
                    eprintln!("{}", serde_json::json!({"event":"rust_odds_joint_cap_experiment", "group":model.request.id,
                    "setup":plan.describe(model),"setup_ms":setup_ms,"elapsed_ms":start.elapsed().as_secs_f64()*1000.,
                    "probability":result.probability,"hits":result.hits,"ess":result.ess,"relative_se":if result.probability>0.{Some(result.std_err/result.probability)}else{None},
                    "check_probability":check.probability,"check_hits":check.hits,"check_ess":check.ess,"accepted":valid,"publication":"deferred"}));
                }
                if valid { result.work=joint_work;native=Some(result);ordinary_draws=10000; }
                else { ordinary_draws=43000; }
            }
        }
'''
  native=native.replace('\\n','\n')
  part=part[:pos]+native+part[pos:]
  part=part.replace('cell.rank, 50000, seed);','cell.rank, ordinary_draws, seed);')
  part=part.replace('        (*index, result)','        result.work += joint_work;\n        (*index, result, native)')
  part=part.replace('    for (i, mut extra) in extras {','    let mut pending = Vec::new();\n    for (i, mut extra, native) in extras {\n        if let Some(r) = native { pending.push((cells[i], r)); }')
  part=part.replace('        results[i].result = extra;\n    }\n}', '        results[i].result = extra;\n    }\n    pending\n}')
  s=s[:start]+part+s[end:]
  pos=s.index('    extra(\n        model,');s=s[:pos]+s[pos:].replace('    extra(', '    let pending_joint = extra(',1)
  pos=s.index('    if witnesses > 0 {')
  late=r'''    let mut joint_found = 0;
    for (cell, result) in pending_joint {
        let est = &mut estimates[cell.index(model.n)];
        if est.probability == 0. && !est.reachability.starts_with("impossible") {
            apply(est, &result, "matched_point_pool_joint_caps");
            witnesses += 1;
            joint_found += 1;
        }
    }
    report("joint_caps_publish", serde_json::json!({"accepted":joint_found,"cells":crate::logging::cell_counts(estimates)}));
'''
  s=s[:pos]+late+s[pos:]
 p.write_text(s)
 subprocess.run(['cargo','build','--release','--offline','--locked','--manifest-path',str(d/'odds-rust/Cargo.toml'),'--bin','golaberto-odds','-j4'],env=dict(__import__('os').environ,CARGO_TARGET_DIR=str(a.target.resolve())),check=True)
if __name__=='__main__':main()
