function unit() { return 1; }
function work(n) {
    var sum=0;
    for(var i=0;i<n;i++) sum=sum+unit();
    return sum;
}
print(work(800000));
