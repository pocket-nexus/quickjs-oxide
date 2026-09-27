globalThis.keep = [];
for (let i = 0; i < 32768; i++) keep.push({x: i, y: i + 1});
print(keep[0].x + keep[32767].y);
