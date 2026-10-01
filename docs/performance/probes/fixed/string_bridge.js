function work(n) {
    var sum='a', suffix='b', completed=0;
    for(var i=0;i<n;i++) {
        sum=sum+suffix;
        if(sum!=='ab') return -1;
        sum='a';
        completed++;
    }
    return completed;
}
print(work(180000));
