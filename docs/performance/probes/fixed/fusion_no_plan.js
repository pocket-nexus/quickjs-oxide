function work(n) {
    var plain=41, sum=0;

    while(n) { sum=plain; n=n-1; }
    return sum;
}
print(work(3000000));
