import pathlib,json,statistics
p=pathlib.Path('/tmp/oxide-six-evidence');d=json.load(open(p/'original-score/results.json'));s=d['samples'];out={}
assert all(not pathlib.Path(x['stderr']).read_bytes() for x in s), 'unexpected stderr'
for case in sorted({x['case'] for x in s}):
 rows=[x for x in s if x['case']==case];pairs=[]
 for rep in sorted({x['repetition'] for x in rows}):
  pair={x['engine']:x for x in rows if x['repetition']==rep};assert len(pair)==2 and all(x['status']=='ok' for x in pair.values())
  pairs.append(pair['after']['measurements']['Score']/pair['before']['measurements']['Score'])
 out[case]={'median_paired_score_ratio':statistics.median(pairs),'paired_score_ratios':pairs,'before_scores':[x['measurements']['Score'] for x in rows if x['engine']=='before'],'after_scores':[x['measurements']['Score'] for x in rows if x['engine']=='after']}
(p/'score-summary.json').write_text(json.dumps(out,indent=2)+'\n');print(out)
