function fault() { var x=1n; x=x+1; return x; }
function work(n) {
    var caught=0;
    for(var i=0;i<n;i++) {
        try { fault(); }
        catch(error) { if(!(error instanceof TypeError)) throw error; caught++; }
    }
    return caught;
}
print(work(12000));
