import pathlib,json,subprocess,hashlib
out=pathlib.Path('/tmp/oxide-six-evidence');p=pathlib.Path('/tmp/oxide-six-accepted-profile');binary='/home/eric/.cache/oxide-six-profile-accepted/release/qjs'
r=subprocess.run(['python3','scripts/benchmark/profile_v8.py','--source','/tmp/oxide-ordinary-v8-source','--engine',binary,'--output',str(p)])
if r.returncode:raise SystemExit(r.returncode)
summary={}
for stage,src in [('baseline',pathlib.Path('/tmp/oxide-six-current-profile')),('provisional-four',pathlib.Path('/tmp/oxide-six-final-profile')),('accepted',p)]:
 summary[stage]={}
 for f in (src/'raw').glob('*.cost.jsonl'):
  ds=[json.loads(l) for l in f.read_text().splitlines()];memory=next(d for d in ds if d['schema']=='oxide-memory-v1');cost=next(d for d in ds if d['schema']=='oxide-compile-vm-cost-v2');summary[stage][f.stem.split('.')[0]]={'events':cost['owned_execution_events'],'layouts':cost['owned_execution_layouts'],'heap_states':memory['heap_states'],'categories':memory['categories'],'metadata':cost['metadata']}
(out/'accepted-profile-summary.json').write_text(json.dumps(summary,indent=2)+'\n')
focused={}
for w in json.load(open(out/'focused-manifest.json'))['metadata']['workloads']['workloads']:
 assert hashlib.sha256(pathlib.Path(w['path']).read_bytes()).hexdigest()==w['sha256']
 target=out/(w['case']+'-accepted.profile.jsonl');cmd=[binary,'-d','--profile-json','--profile-output',str(target),w['path']];r=subprocess.run(cmd,capture_output=True,text=True);assert r.returncode==0 and r.stdout==w['expected'] and not r.stderr
 focused[w['case']]={'command':cmd,'binary_sha256':hashlib.sha256(pathlib.Path(binary).read_bytes()).hexdigest(),'workload_sha256':w['sha256'],'stdout':r.stdout,'stderr':r.stderr,'records':[json.loads(l) for l in target.read_text().splitlines()]}
(out/'accepted-focused-profile.json').write_text(json.dumps(focused,indent=2)+'\n')
