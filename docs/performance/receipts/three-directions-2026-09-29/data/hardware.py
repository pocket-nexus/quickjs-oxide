import json,subprocess,hashlib,statistics,os
from pathlib import Path
out=Path('/tmp/oxide-three-evidence'); raw=out/'hardware';raw.mkdir(exist_ok=False)
items=[]
for f in ['apps-manifest.json','focused-manifest.json']:
 items+=json.loads((out/f).read_text())['metadata']['workloads']['workloads']
selected={'realworld-react','crypto','string-scalar-index','materialized-array-write','native-collections','ordinary-property-write'}
rows=[]
for w in items:
 if w['case'] not in selected:continue
 for rep in range(4):
  for name in (['parent','combined'] if rep%4 in [0,3] else ['combined','parent']):
   prefix=raw/f'{w["case"]}-{name}-{rep}'
   binary=Path('/tmp/oxide-three-build-'+name+'/release/qjs')
   assert hashlib.sha256(Path(w['path']).read_bytes()).hexdigest()==w['sha256']
   cmd=['perf','stat','-x,','-o',str(prefix)+'.csv','-e','instructions:u,cycles:u','--','taskset','-c','2',str(binary),w['path']]
   with Path(str(prefix)+'.stdout').open('wb') as stdout, Path(str(prefix)+'.stderr').open('wb') as stderr:
    proc=subprocess.Popen(cmd,stdout=stdout,stderr=stderr)
    _,status,usage=os.wait4(proc.pid,0)
    proc.returncode=os.waitstatus_to_exitcode(status)
   assert proc.returncode==0 and Path(str(prefix)+'.stdout').read_text()==w['expected'] and not Path(str(prefix)+'.stderr').read_bytes(),w['case']
   values={}
   for line in Path(str(prefix)+'.csv').read_text().splitlines():
    fields=line.split(',')
    if len(fields)>2 and fields[2] in ['instructions:u','cycles:u']: values[fields[2]]=int(fields[0])
   rows.append(dict(case=w['case'],engine=name,repetition=rep,values=values,wrapper_peak_rss_kib=usage.ru_maxrss,command=cmd,binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest()))
   print(w['case'],name,rep,flush=True)
(out/'hardware.json').write_text(json.dumps(rows,indent=2)+'\n')
