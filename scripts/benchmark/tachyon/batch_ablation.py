"""Isolate Tachyon's dispatch-batch effect; restore checkout and baseline ELF afterward."""
from pathlib import Path
import hashlib,json,os,shutil,subprocess,sys
ROOT=Path(__file__).resolve().parents[3]
OUT=ROOT/'target/tachyon-comparison'
def digest(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
    assert (OUT/'control/results.json').exists(), 'finish prior measurements first'
    source=OUT/'source';file=source/'crates/tachyon-vm/src/tuning/dispatch.rs'
    binary=source/'target/release/examples/qxo-bench'
    subprocess.run(['git','diff','--exit-code','HEAD'],cwd=source,check=True)
    receipt=json.loads((OUT/'build.json').read_text())
    assert digest(binary)==receipt['binary_sha256']
    saved=file.read_bytes();needle=b'DEFAULT_DISPATCH_BATCH: usize = 8;'
    assert saved.count(needle)==1
    artifact=OUT/'batch-artifacts';artifact.mkdir(exist_ok=False)
    base=artifact/'tachyon-batch8';variant=artifact/'tachyon-batch1'
    shutil.copy2(binary,base)
    env=os.environ.copy()
    for key in list(env):
        if key in ['RUSTFLAGS','CARGO_ENCODED_RUSTFLAGS','CARGO_BUILD_RUSTFLAGS','RUSTC','RUSTC_WRAPPER','RUSTC_WORKSPACE_WRAPPER','CARGO_TARGET_DIR'] or key.startswith('CARGO_PROFILE_'):env.pop(key)
    cmd=['cargo','+1.95.0','build','--locked','--release','-p','tachyon-vm','--example','qxo-bench']
    try:
        file.write_bytes(saved.replace(needle,b'DEFAULT_DISPATCH_BATCH: usize = 1;'))
        (artifact/'batch1.patch').write_bytes(subprocess.check_output(['git','diff','HEAD'],cwd=source))
        with (artifact/'build.log').open('w') as log:
            subprocess.run(cmd,cwd=source,env=env,stdout=log,stderr=subprocess.STDOUT,check=True)
        shutil.copy2(binary,variant)
        (artifact/'build.json').write_text(json.dumps(dict(commit=receipt['commit'],compiler=receipt['compiler'],command=cmd,adapter_sha256=receipt['adapter_sha256'],patch_sha256=digest(artifact/'batch1.patch'),batch8_sha256=digest(base),batch1_sha256=digest(variant),policy='single dispatch constant 8 -> 1; identical source/compiler/profile otherwise'),indent=2)+'\n')
    finally:
        file.write_bytes(saved)
        shutil.copy2(base,binary)
    assert digest(binary)==receipt['binary_sha256']
    subprocess.run(['git','diff','--exit-code','HEAD'],cwd=source,check=True)
    cmd=[sys.executable,str(ROOT/'scripts/benchmark/fixed.py'),'--manifest',str(ROOT/'docs/reports/data-structure-fixed-final.json'),'--repeat','5','--cpu','2','--engine','batch8='+str(base),'--engine','batch1='+str(variant),'--output',str(OUT/'batch-ablation')]
    for case in ['empty_loop','int_arith','float_arith','func_call','string_length']:cmd+=['--case',case]
    with (OUT/'batch-ablation.log').open('w') as log:subprocess.run(cmd,cwd=ROOT,stdout=log,stderr=subprocess.STDOUT,check=True)
    print('Batch ablation complete; pinned source and baseline binary restored')
if __name__=='__main__':main()
