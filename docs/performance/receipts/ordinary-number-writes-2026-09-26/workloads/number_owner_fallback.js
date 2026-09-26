function work(n) { var x=0, o={value:1}; for(var i=0;i<n;i++) { x=o; x=i; } return x+o.value; } print(work(1000000));
