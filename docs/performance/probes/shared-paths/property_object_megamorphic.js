function read(o) { return o.x; }
var objects=[{x:{answer:7}},{a:0,x:{answer:7}},{a:0,b:0,x:{answer:7}}];
function work(n) { var value; for(var i=0;i<n;i++) value=read(objects[i%3]); return value.answer; }
print(work(200000));
