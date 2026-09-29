import argparse,json,subprocess,pathlib,hashlib,re
p=argparse.ArgumentParser();p.add_argument('--manifest',required=True);p.add_argument('--candidate',required=True);p.add_argument('--group',required=True);p.add_argument('--case',action='append');a=p.parse_args()
out=pathlib.Path('/tmp/oxide-six-evidence');raw=out/a.group;raw.mkdir();items=json.load(open(a.manifest))['metadata']['workloads']['workloads'];rows=[]
bins={'before':pathlib.Path('/tmp/oxide-six-build-baseline/release/qjs'),'after':pathlib.Path(a.candidate)};hashes={k:hashlib.sha256(v.read_bytes()).hexdigest() for k,v in bins.items()}
for w in items:
 if a.case and w['case'] not in a.case:continue
 assert hashlib.sha256(pathlib.Path(w['path']).read_bytes()).hexdigest()==w['sha256']
 for rep in range(4):
  for name in (['before','after'] if rep%4 in [0,3] else ['after','before']):
   binary=bins[name];assert hashlib.sha256(binary.read_bytes()).hexdigest()==hashes[name]
   prefix=raw/f'{w["case"]}-{name}-{rep}';core=['taskset','-c','2',str(binary),w['path']]
   cmd=['perf','stat','-x,','-o',str(prefix)+'.csv','-e','instructions:u,cycles:u','--',*core]
   r=subprocess.run(cmd,capture_output=True,text=True);pathlib.Path(str(prefix)+'.stdout').write_text(r.stdout);pathlib.Path(str(prefix)+'.stderr').write_text(r.stderr)
   assert r.returncode==0 and r.stdout==w['expected'] and not r.stderr,(w['case'],r)
   counters=pathlib.Path(str(prefix)+'.csv').read_text();values={}
   for line in counters.splitlines():
    f=line.split(',')
    if len(f)>2 and f[2] in ['instructions:u','cycles:u']:values[f[2]]=int(f[0])
   assert len(values)==2
   rsscmd=['zsh','-fc','TIMEFMT="rss_kib=%M"; time "$@"','oxide-rss',*core];rss=subprocess.run(rsscmd,capture_output=True,text=True)
   assert rss.returncode==0 and rss.stdout==w['expected'] and re.fullmatch(r'rss_kib=\d+\n',rss.stderr),(w['case'],rss)
   rows.append(dict(case=w['case'],engine=name,repetition=rep,values=values,peak_rss_kib=int(rss.stderr.split('=')[1]),command=cmd,rss_command=rsscmd,binary_sha256=hashes[name],workload_sha256=w['sha256'],perf_raw=counters,rss_raw=rss.stderr));(out/(a.group+'.json')).write_text(json.dumps(rows,indent=2)+'\n')
 print(w['case'],flush=True)
