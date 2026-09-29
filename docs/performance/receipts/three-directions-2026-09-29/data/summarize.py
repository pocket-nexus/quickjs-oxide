import json,statistics,random,math
from pathlib import Path
out=Path('/tmp/oxide-three-evidence'); result={}
for p in sorted(out.glob('*/results.json')):
 d=json.loads(p.read_text()); by={}
 for s in d['samples']:
  assert s['status']=='ok',(p,s)
  by.setdefault(s['case'],{}).setdefault(s['repetition'],{})[s['engine']]=s['process_wall_ns']
 result[p.parent.name]={}
 for case,reps in by.items():
  ratios=[r['after']/r['before'] for r in reps.values()]
  rng=random.Random(29)
  boots=sorted(statistics.median(rng.choices(ratios,k=len(ratios))) for _ in range(4000))
  row={'pairs':len(ratios),'median_paired_ratio':statistics.median(ratios),'min_ratio':min(ratios),'max_ratio':max(ratios),'bootstrap_median_95ci':[boots[100],boots[3899]],'before_median_ns':statistics.median(r['before'] for r in reps.values()),'after_median_ns':statistics.median(r['after'] for r in reps.values())}
  result[p.parent.name][case]=row
 print(p.parent.name, ' '.join(f'{k}={v["median_paired_ratio"]:.4f}' for k,v in result[p.parent.name].items()))
(out/'summary.json').write_text(json.dumps(result,indent=2)+'\n')
