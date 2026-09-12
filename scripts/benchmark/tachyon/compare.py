"""Compare pinned Tachyon host with named engines using the existing fixed corpus."""
import argparse
import json
from pathlib import Path
import resource
import subprocess
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from fixed import load_workloads
from run import binary_metadata, digest, machine_metadata, run_sample
from scaling import admit
from probes import PROBES


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--engine', action='append', required=True)
    p.add_argument('--manifest', type=Path, default=Path('docs/reports/data-structure-fixed-final.json'))
    p.add_argument('--output', type=Path, required=True)
    p.add_argument('--cpu', type=int, default=2)
    p.add_argument('--repeat', type=int, default=5)
    p.add_argument('--profile-repeat', type=int, default=3)
    p.add_argument('--timing-only', action='store_true', help='finish after the complete fixed timing matrix')
    args = p.parse_args()
    engines = {}
    for entry in args.engine:
        name, sep, path = entry.partition('=')
        if not sep or not name.isidentifier() or name in engines:
            p.error('engines need unique identifier=/path entries')
        engines[name] = Path(path).resolve()
    if 'tachyon' not in engines or args.repeat < 1 or args.profile_repeat < 1 or args.cpu < 0:
        p.error('tachyon engine and positive repetitions required')
    out = args.output.resolve()
    out.mkdir(parents=True, exist_ok=False)
    # Bound an experimental engine's process address space; inherited by all comparators.
    resource.setrlimit(resource.RLIMIT_AS, (8*1024**3, 8*1024**3))
    workloads = load_workloads(args.manifest)
    meta = dict(machine=machine_metadata(), engines={k:binary_metadata(v) for k,v in engines.items()},
                workloads=workloads, cpu=args.cpu, address_space_limit_bytes=8*1024**3,
                driver_sha256=digest(__file__), probes_sha256=digest(Path(__file__).with_name('probes.py')))
    (out/'metadata.json').write_text(json.dumps(meta, indent=2)+'\n')
    def record(stage, sample):
        with (out/(stage+'.jsonl')).open('a') as f:
            f.write(json.dumps(sample)+'\n')
    def sample(stage, case, engine, rep, script, expected, prefix_command=None, timeout=180):
        assert digest(engines[engine])==meta['engines'][engine]['sha256']
        directory=out/stage;directory.mkdir(exist_ok=True)
        prefix=directory/f'{case}-{engine}-{rep}'
        cmd=(prefix_command or [])+['taskset','-c',str(args.cpu),str(engines[engine]),str(script)]
        s=run_sample(cmd,out,prefix,timeout)
        status=admit(s,expected.encode())
        if status=='ok' and Path(s['stderr']).read_bytes():status='unexpected-stderr'
        s.update(case=case,engine=engine,repetition=rep,status=status)
        record(stage,s)
        print(stage,case,engine,rep,status,flush=True)
        return s
    probe_dir=out/'probe-source';probe_dir.mkdir()
    for case,(source,expected) in PROBES.items():
        script=probe_dir/(case+'.js');script.write_text(source)
        for engine in engines:sample('probes',case,engine,0,script,expected,timeout=15)
    # Screen all cases, retain failures, and allow one longer retry for timeouts.
    accepted=[]
    for w in workloads:
        s=sample('screen',w['case'],'tachyon',0,w['path'],w['expected'],timeout=30)
        if s['status']=='timeout':
            s=sample('screen',w['case'],'tachyon',1,w['path'],w['expected'],timeout=180)
        if s['status']=='ok':accepted.append(w)
    (out/'accepted.json').write_text(json.dumps([w['case'] for w in accepted],indent=2)+'\n')
    if not accepted:raise SystemExit('No admitted Tachyon workload; raw failures retained')
    cmd=[sys.executable,str(Path(__file__).resolve().parents[1]/'fixed.py'),'--manifest',str(args.manifest.resolve()),'--repeat',str(args.repeat),'--cpu',str(args.cpu),'--output',str(out/'fixed')]
    for k,v in engines.items():cmd+=['--engine',f'{k}={v}']
    for w in accepted:cmd+=['--case',w['case']]
    with (out/'fixed.log').open('w') as log:
        r=subprocess.run(cmd,stdout=log,stderr=subprocess.STDOUT)
    if r.returncode not in (0,1):raise SystemExit(r.returncode)
    print('fixed complete',flush=True)
    if args.timing_only:
        (out/'timing-complete.json').write_text(json.dumps({'fixed_exit_code':r.returncode})+'\n')
        if r.returncode:raise SystemExit(r.returncode)
        return
    fixed=json.loads((out/'fixed/results.json').read_text())
    eligible={(s['case'],s['engine']) for s in fixed['summary'] if s['eligible']}
    for w in accepted:
        if (w['case'],'tachyon') not in eligible:continue
        assert digest(w['path'])==w['sha256']
        for rep in range(args.profile_repeat):
            directory=out/'profiles';directory.mkdir(exist_ok=True)
            data=directory/f"{w['case']}-tachyon-{rep}.data"
            s=sample('profiles',w['case'],'tachyon',rep,w['path'],w['expected'],
                     ['perf','record','--quiet','--per-thread','-m','1024','-e','cycles:u','-c','1000003','--call-graph','dwarf,16384','-o',str(data),'--'])
            if s['status']=='ok':
                with data.with_suffix('.leaf').open('w') as log:
                    subprocess.run(['perf','report','--stdio','--no-inline','--no-children','--percent-limit','0.5','--show-nr-samples','--sort','symbol','--call-graph','none','-i',str(data)],stdout=log,stderr=subprocess.STDOUT,check=True)
        for rep in range(3):
            for engine in engines:
                if (w['case'],engine) not in eligible:continue
                directory=out/'counters';directory.mkdir(exist_ok=True)
                csv=directory/f"{w['case']}-{engine}-{rep}.csv"
                sample('counters',w['case'],engine,rep,w['path'],w['expected'],
                       ['perf','stat','-x',';','-o',str(csv),'-e','instructions:u,cycles:u,branches:u,branch-misses:u','--'])
    (out/'complete.json').write_text(json.dumps({'accepted_cases':len(accepted),'engines':list(engines)},indent=2)+'\n')

if __name__=='__main__':main()
