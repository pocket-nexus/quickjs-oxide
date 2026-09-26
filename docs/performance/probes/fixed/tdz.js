function fault() { target=target+1; let target=1; }
function work(n) {
    var caught=0;
    for(var i=0;i<n;i++) {
        try { fault(); }
        catch(error) { if(!(error instanceof ReferenceError)) throw error; caught++; }
    }
    return caught;
}
print(work(12000));
