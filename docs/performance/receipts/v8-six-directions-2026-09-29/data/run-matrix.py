import argparse,subprocess,json,pathlib,sys
p=argparse.ArgumentParser();p.add_argument('group');p.add_argument('candidate');p.add_argument('--manifest',default='docs/performance/receipts/three-directions-2026-09-29/data/v8-manifest.json');p.add_argument('--repeat',type=int,default=8);p.add_argument('--case',action='append');a=p.parse_args()
root=pathlib.Path.cwd();out=pathlib.Path('/tmp/oxide-six-evidence');manifest=root/a.manifest
cmd=[sys.executable,str(root/'scripts/benchmark/fixed.py'),'--manifest',str(manifest),'--engine','before=/tmp/oxide-six-build-baseline/release/qjs','--engine','after='+a.candidate,'--repeat',str(a.repeat),'--order','abba-baab','--cpu','2','--output',str(out/a.group)]
for c in a.case or []:cmd+=['--case',c]
(out/(a.group+'.command.json')).write_text(json.dumps(cmd,indent=2))
with (out/(a.group+'.log')).open('w') as f:r=subprocess.run(cmd,stdout=f,stderr=subprocess.STDOUT)
print(a.group,r.returncode,flush=True);sys.exit(r.returncode)
