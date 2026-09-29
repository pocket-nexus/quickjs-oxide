// Miss: the absent index must observe the inherited setter on every write.
function work(n) {
    var calls = 0, last = -1, a = [0, , 2];
    var proto = Object.create(Array.prototype);
    Object.defineProperty(proto, '1', {set: function(v) {
        if (this !== a) throw new Error('wrong receiver');
        calls++; last = v;
    }});
    Object.setPrototypeOf(a, proto);
    for (var i = 0; i < n; i++) a[1] = i;
    return calls + ':' + last + ':' + Object.hasOwn(a, '1');
}
print(work(20000));
