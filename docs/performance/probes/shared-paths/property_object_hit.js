function work(o,n) { var value; while(n) { value=o.x; n=n-1; } return value.answer; }
print(work({x:{answer:7}},500000));
