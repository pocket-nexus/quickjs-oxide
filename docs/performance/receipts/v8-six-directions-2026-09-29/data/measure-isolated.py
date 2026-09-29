import subprocess,sys,pathlib,json
root=pathlib.Path('/tmp/oxide-six-evidence');plan=[('aa','/tmp/oxide-six-build-baseline/release/qjs',4),('numeric','/tmp/oxide-six-build-numeric/release/qjs',8),('property','/home/eric/.cache/oxide-six-property/release-target/release/qjs',8),('shape','/home/eric/.cache/oxide-six-build-shape/release/qjs',8),('transition','/home/eric/.cache/oxide-six-build-transition/release/qjs',8)]
for name,binary,count in plan:
 print('start',name,flush=True);cmd=[sys.executable,str(root/'run-matrix.py'),name,binary,'--repeat',str(count)]
 if name=='aa':cmd+=['--manifest',str(root/'v8-nine-manifest.json')]
 subprocess.run(cmd,check=True);subprocess.run([sys.executable,str(root/'summarize.py')],check=True);print('finished',name,flush=True)
