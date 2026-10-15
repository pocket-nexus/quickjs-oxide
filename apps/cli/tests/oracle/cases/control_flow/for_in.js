// case: own integer and string key order excludes symbols
(function(){var o={b:1,2:1,a:1,1:1};o[Symbol.iterator]=1;var s='';for(var k in o)s+=k+',';return s})()
// case: nullish values enumerate nothing
(function(){var n=0;for(var a in null)n++;for(var b in undefined)n++;return n})()
// case: String primitive exposes UTF-16 indices
(function(){var s='';for(var k in 'A\uD83D\uDCA9')s+=k+',';return s})()
// case: non-string primitives box without own keys
(function(){var n=0;for(var a in 1)n++;for(var b in true)n++;for(var c in 1n)n++;for(var d in Symbol.iterator)n++;return n})()
// case: prototype duplicates and nonenumerable own keys are shadowed
(function(){var p={x:1,y:1},o=Object.create(p);Object.defineProperty(o,'x',{value:2,enumerable:false,configurable:true});o.a=1;var s='';for(var k in o)s+=k+',';return s})()
// case: deleted pending keys are skipped and additions miss the snapshot
(function(){var o={a:1,b:2},s='';for(var k in o){s+=k;if(k==='a'){delete o.b;o.c=3}}return s})()
// case: enumerability is captured but presence is live
(function(){var o={a:1,b:2},s='';for(var k in o){s+=k;if(k==='a')Object.defineProperty(o,'b',{enumerable:false})}return s})()
// case: prototype is snapshotted only when reached
(function(){var p={p1:1,p2:2},o=Object.create(p),s='';o.a=1;for(var k in o){s+=k+',';if(k==='a'){delete p.p2;p.p3=3}}return s})()
// case: prototype links are read live between levels
(function(){var p1={x:1},p2={y:1},o=Object.create(p1),s='';o.a=1;for(var k in o){s+=k;if(k==='a')Object.setPrototypeOf(o,p2)}return s})()
// case: deleted snapshotted own key still shadows its prototype duplicate
(function(){var p={x:1},o=Object.create(p),s='';Object.defineProperty(o,'x',{value:2,enumerable:false,configurable:true});o.a=1;for(var k in o){s+=k;if(k==='a')delete o.x}return s})()
// case: ordinary accessors are never invoked by enumeration
(function(){var n=0,o={};Object.defineProperty(o,'x',{enumerable:true,get:function(){n++;return 1}});var s='';for(var k in o)s+=k;return s+'|'+n})()
// case: fast Array deletion exposes an inherited same-name index
(function(){var p=[];p[1]='proto';var a=[0,1],s='';Object.setPrototypeOf(a,p);for(var k in a){s+=k+',';if(k==='0')delete a[1]}return s})()
// case: fast Array additions shadow prototype keys without joining the iteration
(function(){var p={foo:'proto'},a=[0],s='';Object.setPrototypeOf(a,p);for(var k in a){s+=k+',';if(k==='0')a.foo='own'}return s})()
// case: fast Array non-enumerable additions still shadow prototype keys
(function(){var p={foo:'proto'},a=[0],s='';Object.setPrototypeOf(a,p);for(var k in a){s+=k+',';if(k==='0')Object.defineProperty(a,'foo',{value:'own'})}return s})()
// case: defineProperty turns an Array slow and preserves its initial shadow set
(function(){var p=[];p[1]='proto';var a=[0,1],s='';Object.defineProperty(a,'0',{writable:false});Object.setPrototypeOf(a,p);for(var k in a){s+=k+',';if(k==='0')delete a[1]}return s})()
// case: sparse growth keeps an Array slow after the sparse key is deleted
(function(){var p=[];p[1]='proto';var a=[0,1],s='';a[3]=3;delete a[3];Object.setPrototypeOf(a,p);for(var k in a){s+=k+',';if(k==='0')delete a[1]}return s})()
// case: sloppy var initializer executes before the right operand
(function(){var log='',k;for(var k=(log+='i',7) in (log+='r',{a:1}))log+=k;return log+'|'+k})()
// case: for-in right operand accepts a comma Expression
(function(){var k;for(k in (0,{x:1})){}return k})()
// case: fixed and computed member targets are evaluated per key
(function(){var o={value:''},name='value',s='';for(o[name] in {a:1,b:1})s+=o.value;return s+'|'+o.value})()
// case: let head creates fresh captured cells
(function(){var a=[];for(let k in {x:1,y:1})a.push(function(){return k});return a[0]()+'|'+a[1]()})()
// case: labelled continue and break retain then drop the enumeration object
(function(){var s='';outer:for(var k in {a:1,b:1,c:1}){if(k==='a')continue outer;s+=k;if(k==='b')break outer}return s})()
// case: finally runs on continue and break
(function(){var s='';for(var k in {a:1,b:1}){try{if(k==='a')continue;break}finally{s+=k}}return s})()
// case: nested for-in loops retain independent records
(function(){var s='';for(var a in {x:1,y:1})for(var b in {1:1,2:1})s+=a+b;return s})()
