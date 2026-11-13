function read(a,k) {
  try { var value=a[++k]; return String(value)+":"+String(k); }
  catch(e) { return e.name+":"+String(k); }
}
print(read([4,8],0),read({1:9},"0"),read([4,8],0n));
print(read({2147483648:12},2147483647),read(null,0));
var calls=0;
var inherited=Object.create({get 1(){calls++;return 11}});
print(read(inherited,0),calls);
print(read(new Proxy({}, {get:function(o,k){return "key="+k}}),0));
var k=0, o={get 1(){throw new Error("getter")}};
try { o[++k]; } catch(e) { print(e.message,k); }
var key={valueOf:function(){throw new Error("convert")}};
function conversion(a,k) {try{a[++k]}catch(e){print(e.message,k===key)}}
conversion([],key);
function alias(a){return a[++a]}
print(alias({1:22,valueOf:function(){return 0}}));
