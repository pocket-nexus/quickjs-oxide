import json,pathlib,statistics,random,math
out=pathlib.Path('/tmp/oxide-six-evidence');summary={}
for f in sorted(out.glob('*/results.json')):
 d=json.loads(f.read_text());samples=d.get('samples',[])
 if not samples or 'measurements' in samples[0]:continue
 cases={x['case'] for x in samples};s={}
 for case in sorted(cases):
  rr=[x for x in samples if x['case']==case];engines=sorted({x['engine'] for x in rr});
  if engines!=['after','before']:continue
  pairs=[]
  for rep in sorted({x['repetition'] for x in rr}):
   pair={x['engine']:x for x in rr if x['repetition']==rep}
   if len(pair)!=2 or any(x['status']!='ok' for x in pair.values()):raise RuntimeError((f,case,rep,pair))
   pairs.append(pair['after']['process_wall_ns']/pair['before']['process_wall_ns'])
  rng=random.Random(29);res=sorted(statistics.median(rng.choices(pairs,k=len(pairs))) for _ in range(4000));s[case]={'n':len(pairs),'median_paired_ratio':statistics.median(pairs),'range':[min(pairs),max(pairs)],'bootstrap_median_95ci':[res[100],res[3899]],'paired_ratios':pairs,'before_median_ns':statistics.median(x['process_wall_ns'] for x in rr if x['engine']=='before'),'after_median_ns':statistics.median(x['process_wall_ns'] for x in rr if x['engine']=='after')}
 summary[f.parent.name]=s
(out/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
for g,s in summary.items():
 print(g,{c:round(v['median_paired_ratio'],4) for c,v in s.items()})
