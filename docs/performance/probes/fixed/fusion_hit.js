function work(n) {
    var sum=0;
    while(n>0) { sum=sum+1; n=n-1; }
    return sum;
}
print(work(3000000));
