"""Validate and reduce a completed Tachyon comparison without discarding failures."""
import argparse
from collections import defaultdict
import hashlib
import json
from pathlib import Path
import re
import statistics


def digest(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def journal(p):return [json.loads(s) for s in p.read_text().splitlines()] if p.exists() else []

def profiles(directory):
    grouped=defaultdict(list)
    suffix='resolved-leaf' if list(directory.glob('*.resolved-leaf')) else 'leaf'
    for p in sorted(directory.glob('*.'+suffix)):
        text=p.read_text()
        count=re.search(r'Event count \(approx\.\): (\d+)',text)
        lost=re.search(r'Total Lost Samples: (\d+)',text)
        if not count or not lost:raise ValueError(f'invalid perf report: {p}')
        rows=[]
        for line in text.splitlines():
            m=re.match(r'\s*([\d.]+)%\s+(\d+)\s+\[.\]\s+(.*?)\s+-\s+-',line)
            if m:rows.append(dict(percent=float(m[1]),samples=int(m[2]),symbol=m[3]))
        if not rows:raise ValueError(f'empty symbols: {p}')
        case=p.name.removesuffix('.'+suffix).rsplit('-tachyon-',1)[0]
        grouped[case].append(dict(count=int(count[1]),lost=int(lost[1]),rows=rows,file=str(p)))
    result={}
    for case,rs in grouped.items():
        symbols=defaultdict(float);events=sum(r['count'] for r in rs)
        for r in rs:
            for row in r['rows']:symbols[row['symbol']]+=row['percent']*r['count']/events
        result[case]=dict(rounds=len(rs),lost=sum(r['lost'] for r in rs),events=events,
                          top=[dict(symbol=s,percent=v) for s,v in sorted(symbols.items(),key=lambda x:-x[1])],files=[r['file'] for r in rs])
    return result

def timings(path):
    data=json.loads(path.read_text());result=defaultdict(dict)
    for s in data['samples']:
        if s['status']=='ok':
            assert s['exit_code']==0 and not s['timed_out'] and not Path(s['stderr']).read_bytes()
    for r in data['summary']:
        vs=[v/1e6 for v in r['successful_wall_ns']]
        result[r['case']][r['engine']]=dict(eligible=r['eligible'],failures=r['failures'],
            median_ms=statistics.median(vs) if r['eligible'] else None,min_ms=min(vs) if vs else None,max_ms=max(vs) if vs else None,samples_ms=vs)
    return result

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root',type=Path,default=Path('target/tachyon-comparison'))
    args=parser.parse_args();root=args.root.resolve();base=root/'run-final';focus=root/'architecture'
    assert (focus/'complete.json').exists()
    meta=json.loads((base/'metadata.json').read_text())
    expected={w['case']:w['expected'].encode() for w in meta['workloads']}
    for w in meta['workloads']:assert digest(w['path'])==w['sha256']
    for e in meta['engines'].values():assert digest(e['path'])==e['sha256']
    checks={}
    for name,path in [('fixed',base/'fixed/results.json'),('control',root/'control/results.json'),('batch',root/'batch-ablation/results.json')]:
        data=json.loads(path.read_text())
        for s in data['samples']:
            assert s['status']=='ok' and s['exit_code']==0 and not s['timed_out']
            assert Path(s['stdout']).read_bytes()==expected[s['case']]
            assert not Path(s['stderr']).read_bytes()
        for e in data['metadata']['engines'].values():assert digest(e['path'])==e['sha256']
        checks[name+'_valid']=len(data['samples'])
    counters=defaultdict(list)
    for stage in ['profiles','counters']:
        rows=journal(focus/(stage+'.jsonl'))
        for r in rows:
            assert r['status']=='ok' and r['exit_code']==0 and not r['timed_out']
            assert Path(r['stdout']).read_bytes()==expected[r['case']]
            assert not Path(r['stderr']).read_bytes()
            if stage=='counters':
                p=focus/'counters'/f"{r['case']}-{r['engine']}-{r['repetition']}.csv"
                cs={}
                for line in p.read_text().splitlines():
                    if not line or line.startswith('#'):continue
                    cells=line.split(';')
                    if len(cells)>4:
                        assert '<' not in cells[0] and float(cells[4])>=99.9,(p,line)
                        cs[cells[2]]=float(cells[0])
                assert len(cs)==4
                counters[(r['case'],r['engine'])].append(cs)
        checks[stage+'_valid']=len(rows)
    reduced=defaultdict(dict)
    for (case,engine),rs in counters.items():
        assert len(rs)==3
        reduced[case][engine]={k:statistics.median(r[k] for r in rs) for k in rs[0]}
    ps=profiles(focus/'profiles')
    assert len(ps)==8 and all(r['rounds']==3 and r['lost']==0 for r in ps.values())
    checks['lost_samples']=sum(r['lost'] for r in ps.values())
    probes=[]
    for r in journal(base/'probes.jsonl'):
        probes.append(dict(case=r['case'],engine=r['engine'],status=r['status'],stdout=Path(r['stdout']).read_text(),stderr=Path(r['stderr']).read_text()))
    result=dict(build=json.loads((root/'build.json').read_text()),
        compiler_control_build=json.loads((root/'oxide-control-build.json').read_text()),
        batch_build=json.loads((root/'batch-artifacts/build.json').read_text()),
        architecture_probe=json.loads((root/'architecture-probe.json').read_text()),
        machine=meta['machine'],engines=meta['engines'],
        timing=timings(base/'fixed/results.json'),control=timings(root/'control/results.json'),
        batch=timings(root/'batch-ablation/results.json'),profiles=ps,counters=reduced,probes=probes,verification=checks,
        policy='870 full fixed timing samples; user narrowed analysis to architecture; only 8 focused profiles/counters; interrupted broad diagnostic phase excluded')
    (root/'summary.json').write_text(json.dumps(result,indent=2)+'\n')
    print(json.dumps(checks))

if __name__=='__main__':main()
