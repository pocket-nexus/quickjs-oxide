// Per-iteration bindings: each closure sees its own iteration's `i`.
var fns = [];
for (let i = 0; i < 3; i++) fns[i] = function () { return i; };
print(fns[0](), fns[1](), fns[2]());
var arrows = [];
for (let i = 0; i < 3; i++) {
    let j = i * 2;
    arrows[i] = () => i + j;
}
print(arrows[0](), arrows[1](), arrows[2]());
var shared = [];
for (var k = 0; k < 3; k++) shared[k] = () => k;
print(shared[0](), shared[2]());
var bump = [];
for (let n = 0; n < 4; n++) {
    bump[n] = () => ++n;
    if (n === 1) n++;
}
print(bump[0](), bump[0](), bump[1](), bump[3] === undefined);
let outer = 'o';
{
    let outer = 'inner';
    var read = () => outer;
}
print(read(), outer);
