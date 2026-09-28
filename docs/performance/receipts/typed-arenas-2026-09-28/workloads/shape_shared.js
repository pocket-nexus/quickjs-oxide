globalThis.keep = [];
for (let i = 0; i < 4096; i++) keep.push({x: i, y: i + 1});
print(keep[0].x + keep[4095].y);
