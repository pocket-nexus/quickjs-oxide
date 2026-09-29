import json, subprocess, sys
from pathlib import Path
root=Path.cwd(); out=Path('/tmp/oxide-three-evidence')
base='/tmp/oxide-three-build-parent/release/qjs'
bins={n:f'/tmp/oxide-three-build-{n}/release/qjs' for n in ['string-final','array','native','combined','pr52']}
focused=json.loads((out/'focused-manifest.json').read_text())['metadata']['workloads']['workloads']
apps=json.loads((out/'apps-manifest.json').read_text())['metadata']['workloads']['workloads']
v8=json.loads((out/'v8-manifest.json').read_text())['metadata']['workloads']['workloads']
def manifest(name, cases):
 p=out/(name+'-manifest.json'); p.write_text(json.dumps({'metadata':{'workloads':{'workloads':cases}}},indent=2));return p
plans=[('aa',base,base,4,apps[-1:]+[x for x in apps if x['case']=='realworld-react']+[x for x in focused if x['case'] in ['string-scalar-index','native-numeric','materialized-array-write']]),
 ('string-isolated',base,bins['string-final'],12,[x for x in focused if x['case'].startswith('string-')]+[x for x in apps if x['case']=='realworld-react']),
 ('array-isolated',base,bins['array'],12,[x for x in focused if 'array-' in x['case'] or x['case']=='ordinary-property-write']+apps[-1:]),
 ('native-isolated',base,bins['native'],12,[x for x in focused if x['case'].startswith('native-')]+[x for x in apps if x['case']=='realworld-react']),
 ('combined-focused',base,bins['combined'],8,focused),
 ('combined-apps',base,bins['combined'],16,apps),
 ('combined-v8',base,bins['combined'],8,v8),
 ('cumulative-apps',bins['pr52'],bins['combined'],8,apps),
 ('cumulative-v8',bins['pr52'],bins['combined'],4,v8)]
for name,a,b,reps,cases in plans:
 cmd=[sys.executable,str(root/'scripts/benchmark/fixed.py'),'--manifest',str(manifest(name,cases)),'--engine','before='+a,'--engine','after='+b,'--repeat',str(reps),'--order','abba-baab','--cpu','2','--output',str(out/name)]
 print(name,flush=True)
 with (out/(name+'.log')).open('w') as log: subprocess.run(cmd,check=True,stdout=log,stderr=subprocess.STDOUT)
print('MATRIX COMPLETE',flush=True)
