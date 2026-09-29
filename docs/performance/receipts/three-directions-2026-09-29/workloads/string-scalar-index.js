function work(n) {
    var text = 'abcdefgh', sum = 0;
    for (var i = 0; i < n; i++) sum += text.charCodeAt(i & 7);
    return sum;
}
var result = work(400000);
if (result !== 40200000) throw new Error('wrong scalar index checksum: ' + result);
print(result);
