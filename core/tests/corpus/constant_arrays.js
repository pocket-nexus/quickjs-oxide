function make(){return [0,1,2,3,4,5,6,7,8,9]}
var a=make(),b=make();a[0]=99;b.push(10);
print(a.length,b.length,a[0],b[0],a===b);
function values(){return [undefined,null,true,false,1,1.5,2n,"中"]}
print(values().map(String).join("|"));
var sparse=[1,,3];print(sparse.length,1 in sparse,2 in sparse);
var order=[];function effect(n){order.push(n);return n}
print([effect(1),effect(2)].join(":"),order.join(":"));
function nested(){return [[1,2],[3,4]]}
var x=nested(),y=nested();x[0][0]=8;print(y[0][0],x[0]===y[0]);
