import subprocess,os,pathlib,json
out=pathlib.Path('/tmp/oxide-six-evidence');env={k:v for k,v in os.environ.items() if not k.startswith('GIT_')};env.update(RUSTUP_TOOLCHAIN='1.88.0',CARGO_TARGET_DIR='/home/eric/.cache/oxide-six-validation',CARGO_BUILD_JOBS='2',TEST262_WORKERS='2')
commands=[('fmt',['cargo','fmt','--all','--','--check']),('source-layout',['python3','scripts/checks/check-source-layout.py']),('rust-only',['bash','scripts/checks/check-rust-only.sh']),('workspace-tests',['cargo','test','--locked','--workspace','--all-targets','--jobs','2']),('profile-tests',['cargo','test','--locked','-p','quickjs-oxide','--lib','--features','profiling','--jobs','2']),('clippy',['cargo','clippy','--locked','--workspace','--all-targets','--all-features','--jobs','2','--','-D','warnings']),('test262-check',['bash','scripts/test262/test-test262.sh','--check']),('test262-full',['bash','scripts/test262/test-test262.sh','--full'])]
rows=[]
for name,cmd in commands:
 print('start',name,flush=True)
 with (out/(name+'.log')).open('w') as f:r=subprocess.run(cmd,env=env,stdout=f,stderr=subprocess.STDOUT)
 rows.append({'name':name,'command':cmd,'exit':r.returncode});(out/'validation.json').write_text(json.dumps(rows,indent=2));print('finish',name,r.returncode,flush=True)
 if r.returncode:raise SystemExit(r.returncode)
