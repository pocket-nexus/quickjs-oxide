import pathlib,subprocess,sys,json
out=pathlib.Path('/tmp/oxide-six-evidence');binary=sys.argv[1]
steps=[
 ('accepted-fixed',[sys.executable,str(out/'run-matrix.py'),'accepted',binary,'--repeat','8','--manifest',str(out/'v8-nine-manifest.json')]),
 ('accepted-apps',[sys.executable,str(out/'run-matrix.py'),'accepted-apps',binary,'--repeat','8','--manifest',str(out/'apps-manifest.json'),'--case','realworld-react','--case','realworld-solid','--case','realworld-vue']),
 ('accepted-focused',[sys.executable,str(out/'run-matrix.py'),'accepted-focused',binary,'--repeat','8','--manifest',str(out/'focused-manifest.json')]),
 ('hardware',[sys.executable,str(out/'hardware-rss.py'),'--manifest',str(out/'v8-nine-manifest.json'),'--candidate',binary,'--group','accepted-hardware']),
 ('focused-hardware',[sys.executable,str(out/'hardware-rss.py'),'--manifest',str(out/'focused-manifest.json'),'--candidate',binary,'--group','accepted-focused-hardware']),
 ('original-score',['taskset','-c','2',sys.executable,'scripts/benchmark/run.py','--suite','v8-v7','--source','/tmp/oxide-ordinary-v8-source','--engine','before=/tmp/oxide-six-build-baseline/release/qjs','--engine','after='+binary,'--case','earley-boyer','--case','raytrace','--case','richards','--case','crypto','--case','deltablue','--case','splay','--repeat','2','--order','abba','--timeout','240','--output',str(out/'original-score')])]
(out/'final-measure-commands.json').write_text(json.dumps(steps,indent=2))
for name,cmd in steps:
 print('start',name,flush=True)
 with (out/(name+'.orchestrator.log')).open('w') as f:r=subprocess.run(cmd,stdout=f,stderr=subprocess.STDOUT)
 print('finish',name,r.returncode,flush=True)
 if r.returncode:sys.exit(r.returncode)
