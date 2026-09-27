function f(n){var a=[1,2,3,4],i=0,j=0,scale=2,sum=0;for(;i<n;i++){j=i&3;sum += a[j]*scale;}return sum;}print(f(10000));
