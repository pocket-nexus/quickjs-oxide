function work(n) {
    var a=[1,2,3,4], sum=0;
    for(var i=0;i<n;i++) sum=sum+a[i&3];
    return sum;
}
print(work(2000000));
