function work(n) {
    var index = 0, calls = 0, sum = 0;
    var receiver = { toString: function() { calls++; return 'abcdefgh'; } };
    var position = { valueOf: function() { calls++; return index; } };
    for (var i = 0; i < n; i++) {
        index = i & 7;
        sum += String.prototype.charCodeAt.call(receiver, position);
    }
    if (calls !== n * 2) throw new Error('wrong conversion count');
    return sum;
}
var result = work(20000);
if (result !== 2010000) throw new Error('wrong scalar object checksum: ' + result);
print(result);
