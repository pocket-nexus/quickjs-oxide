function work(o,n) { var value; while(n) { value=o.x; n=n-1; } return value; }
print(work({x:7},500000));
