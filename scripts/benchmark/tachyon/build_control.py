"""Build the PR19 Oxide source with Tachyon's Rust/release settings as a control."""
import io
import json
import os
from pathlib import Path
import subprocess
import tarfile

ROOT=Path(__file__).resolve().parents[3]
OUT=ROOT/'target/tachyon-comparison'
COMMIT='1cc51bb5fcc5c36912d3197d877219ae513dc4b5'

def main():
    # Never let a diagnostic build overlap the primary timing/profile campaign.
    assert (OUT/'architecture/complete.json').exists()
    source=OUT/'oxide-control-source'
    source.mkdir(exist_ok=False)
    data=subprocess.check_output(['git','archive',COMMIT],cwd=ROOT)
    with tarfile.open(fileobj=io.BytesIO(data)) as archive:
        archive.extractall(source,filter='data')
    env=os.environ.copy()
    for key in list(env):
        if key in ['RUSTFLAGS','CARGO_ENCODED_RUSTFLAGS','CARGO_BUILD_RUSTFLAGS','RUSTC','RUSTC_WRAPPER','RUSTC_WORKSPACE_WRAPPER','CARGO_TARGET_DIR'] or key.startswith('CARGO_PROFILE_'):
            env.pop(key)
    env.update(CARGO_PROFILE_RELEASE_LTO='thin',CARGO_PROFILE_RELEASE_CODEGEN_UNITS='1',CARGO_PROFILE_RELEASE_DEBUG='2',CARGO_PROFILE_RELEASE_PANIC='abort',QUICKJS_OXIDE_BUILD_COMMIT=COMMIT)
    cmd=['cargo','+1.95.0','build','--locked','--release','-p','quickjs-oxide-cli','--no-default-features','--jobs','1']
    with (OUT/'oxide-control-build.log').open('w') as log:
        subprocess.run(cmd,cwd=source,env=env,stdout=log,stderr=subprocess.STDOUT,check=True)
    import hashlib
    digest=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
    binary=source/'target/release/qjs'
    (OUT/'oxide-control-build.json').write_text(json.dumps({'commit':COMMIT,'command':cmd,'compiler':subprocess.check_output(['rustup','run','1.95.0','rustc','-Vv'],text=True),'binary':str(binary),'binary_sha256':digest(binary),'cargo_lock_sha256':digest(source/'Cargo.lock'),'settings':{k:v for k,v in env.items() if k.startswith('CARGO_PROFILE_')},'policy':'diagnostic build control; same PR19 code; not the ordinary historical baseline'},indent=2)+'\n')
    print(binary)

if __name__=='__main__':main()
