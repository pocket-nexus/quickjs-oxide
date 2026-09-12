from pathlib import Path
import sys,json,subprocess,resource
ROOT=Path.cwd();sys.path.insert(0,str(ROOT/'scripts/benchmark'))
from run import digest,run_sample
from scaling import admit
BASE=ROOT/'target/tachyon-comparison/run-final';OUT=ROOT/'target/tachyon-comparison/architecture';OUT.mkdir(exist_ok=False)
meta=json.loads((BASE/'metadata.json').read_text())
cases=['empty_loop','int_arith','func_call','global_read','prop_read','string_length','v8-crypto','v8-earley-boyer']
meta['workloads']=[w for w in meta['workloads'] if w['case'] in cases];meta['driver_sha256']=digest(__file__)
(OUT/'metadata.json').write_text(json.dumps(meta,indent=2)+'\n')
resource.setrlimit(resource.RLIMIT_AS,(8*1024**3,8*1024**3))
for stage in ['profiles','counters']:(OUT/stage).mkdir()
def sample(stage,w,e,rep,cmd):
 assert digest(meta['engines'][e]['path'])==meta['engines'][e]['sha256'];assert digest(w['path'])==w['sha256']
 prefix=OUT/stage/f"{w['case']}-{e}-{rep}"
 r=run_sample(cmd+['taskset','-c','2',meta['engines'][e]['path'],w['path']],ROOT,prefix,180)
 r.update(case=w['case'],engine=e,repetition=rep,status=admit(r,w['expected'].encode()))
 if r['status']=='ok' and Path(r['stderr']).read_bytes():r['status']='unexpected-stderr'
 with (OUT/(stage+'.jsonl')).open('a') as f:f.write(json.dumps(r)+'\n')
 assert r['status']=='ok',r
 print(stage,w['case'],e,rep,flush=True)
for w in meta['workloads']:
 for rep in range(3):
  data=OUT/'profiles'/f"{w['case']}-tachyon-{rep}.data"
  sample('profiles',w,'tachyon',rep,['perf','record','--quiet','--per-thread','-m','1024','-e','cycles:u','-c','1000003','--call-graph','dwarf,16384','-o',str(data),'--'])
  with data.with_suffix('.resolved-leaf').open('w') as f:
   subprocess.run(['perf','--buildid-dir',str(ROOT/'target/profile-refresh/symbol-cache'),'report','--stdio','--no-inline','--no-children','--percent-limit','0','--show-nr-samples','--sort','symbol','--call-graph','none','-i',str(data)],stdout=f,stderr=subprocess.STDOUT,check=True)
for w in meta['workloads']:
 for rep in range(3):
  es=list(meta['engines']);es=es[rep%3:]+es[:rep%3]
  for e in es:
   csv=OUT/'counters'/f"{w['case']}-{e}-{rep}.csv"
   sample('counters',w,e,rep,['perf','stat','-x',';','-o',str(csv),'-e','instructions:u,cycles:u,branches:u,branch-misses:u','--'])
(OUT/'complete.json').write_text(json.dumps({'cases':cases,'profiles':24,'counter_runs':72}))
