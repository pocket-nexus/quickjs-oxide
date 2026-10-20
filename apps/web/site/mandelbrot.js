// mandelbrot.js: the part quickjs-oxide runs.
// The host calls setView() once, then renderBand() pass by pass: first every
// 8th pixel, then every 4th, 2nd and finally all of them. Each pixel is
// computed exactly once; a coarse pass is painted as blocks until the finer
// passes fill it in.
//
// Every point comes back as one character: 0 inside the set, 1-255 a shade,
// 256 already computed in an earlier pass.

var KEEP = 256;
var view = null;
var iterations = 0;

function setView(width, height, cx, cy, span, maxIter, firstStep) {
  view = { width: width, height: height, cx: cx, cy: cy,
           scale: span / width, maxIter: maxIter, firstStep: firstStep };
  iterations = 0;
}

// The points of pass `step` in rows y0 .. y0 + rows - 1.
function renderBand(step, y0, rows) {
  var w = view.width, h = view.height, maxIter = view.maxIter;
  var scale = view.scale, coarse = step * 2, out = [];
  var row = new Array(Math.ceil(w / step));
  for (var y = y0; y < y0 + rows && y < h; y += step) {
    var ci = view.cy + (y - h / 2) * scale;
    var oldRow = step < view.firstStep && y % coarse === 0;
    for (var x = 0, i = 0; x < w; x += step, i++) {
      if (oldRow && x % coarse === 0) {
        row[i] = KEEP;
        continue;
      }
      var cr = view.cx + (x - w / 2) * scale;
      var zr = 0, zi = 0, zr2 = 0, zi2 = 0, n = 0;
      while (n < maxIter && zr2 + zi2 <= 256) {
        zi = 2 * zr * zi + ci;
        zr = zr2 - zi2 + cr;
        zr2 = zr * zr;
        zi2 = zi * zi;
        n++;
      }
      iterations += n;
      row[i] = n === maxIter ? 0 : shade(n, zr2 + zi2);
    }
    out.push(String.fromCharCode.apply(null, row));
  }
  return out.join("");
}

// Smooth escape count, folded into bands that brighten toward the set.
function shade(n, m) {
  var smooth = n + 1 - Math.log(Math.log(m) / 2) / Math.LN2;
  var f = (Math.sqrt(Math.max(0, smooth)) * 0.45) % 1;
  return 1 + Math.floor((1 - Math.abs(2 * f - 1)) * 254);
}
