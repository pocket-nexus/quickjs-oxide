// Nonparticipant: ordinary named-property writes use their existing selector.
function work(n) {
    var o = {x: 0};
    for (var i = 0; i < n; i++) o.x = i;
    return o.x;
}
print(work(200000));
