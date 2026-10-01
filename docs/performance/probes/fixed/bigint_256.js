function work(n) {
    var baseline=(1n << 256n)-1n, x=baseline;
    while(n>0) {
        x=((x ^ 5n)+1n)-1n;
        x=x ^ 5n;
        n=n-1;
    }
    return x===baseline;
}
print(work(40000));
