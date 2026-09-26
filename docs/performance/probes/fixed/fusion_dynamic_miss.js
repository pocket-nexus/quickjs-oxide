function work(n) {
    var calls=0;
    var object={valueOf:function(){calls++;return 1;}};
    var sum=object;
    while(n>0) { sum=sum+1; sum=object; n=n-1; }
    return calls;
}
print(work(180000));
