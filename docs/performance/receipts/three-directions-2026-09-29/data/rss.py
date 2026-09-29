import json,subprocess,re
from pathlib import Path
out=Path('/tmp/oxide-three-evidence');rows=[]
items=[]
for f in ['apps-manifest.json','focused-manifest.json']:items+=json.loads((out/f).read_text())['metadata']['workloads']['workloads']
for w in items:
 if w['case'] not in {'realworld-react','crypto','string-scalar-index','materialized-array-write','native-collections','ordinary-property-write'}:continue
 for rep in range(4):
  for name in (['parent','combined'] if rep%4 in [0,3] else ['combined','parent']):
   cmd=['zsh','-fc','TIMEFMT="rss_kib=%M"; time "$@"','oxide-rss','taskset','-c','2','/tmp/oxide-three-build-'+name+'/release/qjs',w['path']]
   r=subprocess.run(cmd,capture_output=True,text=True,check=True)
   assert r.stdout==w['expected'] and re.fullmatch(r'rss_kib=\d+\n',r.stderr),(w['case'],r)
   rows.append(dict(case=w['case'],engine=name,repetition=rep,peak_rss_kib=int(r.stderr.split('=')[1]),command=cmd))
 print(w['case'],flush=True)
(out/'rss.json').write_text(json.dumps(rows,indent=2)+'\n')
