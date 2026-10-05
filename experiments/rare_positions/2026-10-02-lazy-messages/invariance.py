import json,os,subprocess,hashlib,sys
from pathlib import Path
root=Path(sys.argv[1]).resolve();out=Path(sys.argv[3]).resolve();out.mkdir(parents=True,exist_ok=True)
env={k:v for k,v in os.environ.items()if not k.startswith(('RUST_ODDS_','RARE_POSITION_'))}
env.update(RUST_ODDS_EXPERIMENT_LAZY_MESSAGES='1',RUST_ODDS_EXPERIMENT_LAZY_MESSAGE_ROOTS='1')
request=Path(sys.argv[2]).resolve()
rows=[]
for i,(workers,logging)in enumerate([(4,'1'),(4,'1'),(1,'0')]):
 path=out/f'export-{i}.json'
 with(out/f'run-{i}.log').open('wb')as log:
  subprocess.run([str(root),'estimate',str(request),str(path),'808',str(workers)],env=dict(env,RUST_ODDS_LOG=logging),stdout=log,stderr=log,check=True)
 rows.append({'workers':workers,'logging':logging,'sha256':hashlib.sha256(path.read_bytes()).hexdigest()})
assert len({r['sha256']for r in rows})==1
(out/'summary.json').write_text(json.dumps(rows,indent=2)+'\n');print(json.dumps(rows))
