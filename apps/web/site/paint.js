// paint.js: the page. It paints each band the worker sends.
const STOPS = [
  [0.00, 23, 18, 38], [0.40, 40, 26, 80], [0.50, 120, 70, 200],
  [0.58, 63, 208, 232], [0.68, 255, 95, 158], [0.80, 255, 180, 92],
  [0.90, 255, 210, 63], [1.00, 255, 246, 220],
];
const INSIDE = [18, 12, 33];

// Shade 0 is inside the set; shades 1-255 walk the color stops.
const LUT = Array.from({ length: 256 }, (_, shade) => {
  if (shade === 0) return INSIDE;
  const t = (shade - 1) / 254;
  let i = 0;
  while (i < STOPS.length - 2 && t > STOPS[i + 1][0]) i++;
  const [t0, ...c0] = STOPS[i], [t1, ...c1] = STOPS[i + 1];
  const f = Math.min(1, Math.max(0, (t - t0) / (t1 - t0)));
  return c0.map((v, k) => Math.round(v + (c1[k] - v) * f));
});

// A band of pass `step` holds every step-th point of its rows; each point
// fills a step × step block until a finer pass overwrites it. 256 means the
// point was painted by a coarser pass and stays.
function paint(frame, band) {
  const { width, height, data } = frame;
  const { step, y0, shades } = band, cols = Math.ceil(width / step);
  for (let k = 0; k < shades.length; k++) {
    const shade = shades.charCodeAt(k);
    if (shade === 256) continue;
    const c = LUT[shade], x0 = (k % cols) * step, top = y0 + Math.floor(k / cols) * step;
    for (let y = top; y < Math.min(top + step, height); y++) {
      for (let x = x0; x < Math.min(x0 + step, width); x++) {
        const o = (y * width + x) * 4;
        data[o] = c[0]; data[o + 1] = c[1]; data[o + 2] = c[2]; data[o + 3] = 255;
      }
    }
  }
}
