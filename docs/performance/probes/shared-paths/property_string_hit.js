function work(o,n) { var value; while(n) { value=o.x; n=n-1; } return value.length; }
print(work({x:'abcdefg'},500000));
