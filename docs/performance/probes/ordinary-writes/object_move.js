function work(n) { var x={value:1}, y={value:2}; for(var i=0;i<n;i++) { x=y; } return x.value; } print(work(1000000));
