function work(n) {
    var a=[0,0,0,0];
    for(var i=0;i<n;i++) a[i&3]=i;
    return a[0]+a[1]+a[2]+a[3];
}
print(work(2000000));
