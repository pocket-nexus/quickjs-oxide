// Regular-expression literals: a new object per evaluation,
// `lastIndex` as an own writable property.
function make() { return /ab+c/gi; }
var first = make(), second = make();
print(first === second, typeof first, first.lastIndex, second.lastIndex);
first.lastIndex = 5;
print(first.lastIndex, second.lastIndex);
print('' + /x\/y/, '' + /a/g, '' + /[a-z]+/im);
var all = [/a/, /b/g, /c/y];
print(all.length, all[1].lastIndex, 'lastIndex' in all[2]);
var seen = {};
for (var i = 0; i < 3; i++) { var r = /same/; r.tag = i; seen[i] = r; }
print(seen[0] === seen[1], seen[0].tag, seen[2].tag);
