"""Build an immutable Tachyon checkout with our minimal shell adapter."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess

COMMIT = '2d148e462233c884d0547d4ec0ccc8ccaa183f17'
REPOSITORY = 'https://github.com/tachyon-engine/tachyon-engine.git'
TOOLCHAIN = '1.95.0'

def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()

def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--output', type=Path, default=Path('target/tachyon-comparison'))
    args = p.parse_args()
    out = args.output.resolve()
    out.mkdir(parents=True, exist_ok=True)
    source = out / 'source'
    if not source.exists():
        subprocess.run(['git', 'clone', '--no-checkout', REPOSITORY, str(source)], check=True)
        subprocess.run(['git', '-C', str(source), 'checkout', '--detach', COMMIT], check=True)
    actual = subprocess.check_output(['git', '-C', str(source), 'rev-parse', 'HEAD'], text=True).strip()
    if actual != COMMIT:
        raise SystemExit(f'wrong Tachyon source: {actual}')
    subprocess.run(['git', '-C', str(source), 'diff', '--exit-code', 'HEAD'], check=True)
    adapter = Path(__file__).with_name('main.rs')
    dest = source / 'crates/tachyon-vm/examples/qxo-bench.rs'
    dest.parent.mkdir(parents=True, exist_ok=True)
    if not dest.exists() or digest(dest) != digest(adapter):
        shutil.copyfile(adapter, dest)
    env = os.environ.copy()
    # Pin the release configuration rather than inherit session-specific profiling flags.
    for key in list(env):
        if key in ('RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS', 'CARGO_BUILD_RUSTFLAGS', 'RUSTC', 'RUSTC_WRAPPER', 'RUSTC_WORKSPACE_WRAPPER', 'RUSTUP_TOOLCHAIN', 'CARGO_TARGET_DIR') or key.startswith('CARGO_PROFILE_'):
            env.pop(key)
    command = ['cargo', '+' + TOOLCHAIN, 'build', '--locked', '--release', '-p', 'tachyon-vm', '--example', 'qxo-bench']
    with (out / 'build.log').open('w') as log:
        subprocess.run(command, cwd=source, env=env, stdout=log, stderr=subprocess.STDOUT, check=True)
    binary = source / 'target/release/examples/qxo-bench'
    receipt = dict(repository=REPOSITORY, commit=COMMIT, command=command,
                   compiler=subprocess.check_output(['rustup', 'run', TOOLCHAIN, 'rustc', '-Vv'], text=True),
                   adapter_sha256=digest(adapter), cargo_lock_sha256=digest(source/'Cargo.lock'),
                   binary=str(binary), binary_sha256=digest(binary),
                   profile='upstream release; thin LTO; codegen-units=1; debug=2; panic=abort; no opcode-profile',
                   limits=dict(heap_bytes=2*1024**3, atom_entries=1048576, atom_bytes=64*1024**2,
                               stack_frames=16384, stack_registers=4*1024**2, modules=1024, globals=65536),
                   host='wall clock; JS print output capture; original workload body retained verbatim')
    (out / 'build.json').write_text(json.dumps(receipt, indent=2)+'\n')
    print(binary)

if __name__ == '__main__':
    main()
