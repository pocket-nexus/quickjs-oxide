import pathlib,json,hashlib
p=pathlib.Path('/tmp/oxide-six-evidence');counts={}
for group,expected in [('aa',72),('numeric',128),('property',128),('shape',128),('transition',128),('stack',128),('accepted',144),('accepted-apps',48),('accepted-focused',32),('original-score',24)]:
 d=json.load(open(p/group/'results.json'));s=d['samples'];assert len(s)==expected,(group,len(s));assert all(x['status']=='ok' for x in s),group;assert all(not pathlib.Path(x['stderr']).read_bytes() for x in s),group;counts[group]=len(s)
for name in ['accepted-hardware','accepted-focused-hardware']:
 rows=json.load(open(p/(name+'.json')))
 for r in rows:
  prefix=p/name/(r['case']+'-'+r['engine']+'-'+str(r['repetition']))
  assert not pathlib.Path(str(prefix)+'.stderr').read_bytes()
  r['stdout']=pathlib.Path(str(prefix)+'.stdout').read_text();r['stderr']=''
 (p/(name+'-completion.json')).write_text(json.dumps(rows,indent=2)+'\n')
checks={}
for name,path in [('plain','/home/eric/.cache/oxide-six-build-accepted/release/qjs'),('profiling','/home/eric/.cache/oxide-six-profile-accepted/release/qjs')]:
 f=pathlib.Path(path);d=json.load(open(str(f)+'.build.json'));sha=hashlib.sha256(f.read_bytes()).hexdigest();assert sha==d['binary_sha256'];assert d['commit']=='ee999771e2943ec31c303209f53c1107cf829eda';checks[name]={'commit':d['commit'],'binary_sha256':sha}
prof=json.load(open(p/'accepted-profile-summary.json'));equal={c:{'heap_states':d['heap_states']==prof['baseline'][c]['heap_states'],'layouts':d['layouts']==prof['baseline'][c]['layouts']} for c,d in prof['accepted'].items()};assert all(all(v.values()) for v in equal.values())
v=json.load(open(p/'validation.json'));assert len(v)==8 and all(r['exit']==0 for r in v)
(p/'audit.json').write_text(json.dumps({'successful_samples':counts,'sample_total':sum(counts.values()),'builds':checks,'profile_equal':equal,'validation_gates':len(v)},indent=2)+'\n');print('audit passed',sum(counts.values()),'samples')
