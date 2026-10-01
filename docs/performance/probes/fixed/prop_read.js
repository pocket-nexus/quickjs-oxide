function work(n) {
    var o={x:3}, sum=0;
    for(var i=0;i<n;i++) sum=sum+o.x;
    return sum;
}
print(work(2000000));
