var order=[];function mark(n){order.push(n);return n}
function make(n){return {left:n,right:n+1}}
var first=make(1),second=make(2);first.left=9;print(second.left,second.right,first===second);
var nested={a:mark(1),b:{left:mark(2),right:mark(3)}};print(order.join(":"),Object.keys(nested).join(":"));
var duplicate={a:1,a:2,b:3};print(duplicate.a,Object.keys(duplicate).join(":"));
var computed={a:1,b:2,["a"]:3};print(computed.a,Object.keys(computed).join(":"));
var p={inherited:4},proto={__proto__:p,a:1,b:2};print(proto.inherited,Object.getPrototypeOf(proto)===p);
var method={a:1,b:2,m(){return this.a}};print(method.m());
var accessor={get a(){return 7},b:2};print(accessor.a);
var numeric={0:1,a:2,b:3};print(Object.keys(numeric).join(":"));
function branch(c){return {a:c?1:2,b:3}}print(branch(false).a);
var count=0;Object.defineProperty(Object.prototype,"literalTest",{set:function(){count++},configurable:true});
function inheritedSetter(){return {literalTest:4,other:5}}
print(inheritedSetter().literalTest,count);delete Object.prototype.literalTest;
