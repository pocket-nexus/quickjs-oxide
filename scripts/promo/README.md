# Eight-leg relay video

`v8_race.py` renders **only the V8 v7 relay**: starting lights, eight legs on a
scrolling track, the finish and a held result in the same scene. The cut is
26 seconds at 1920×1080, 60 fps. A native 480×270 canvas is enlarged 4× with
nearest-neighbor sampling and a fixed 12px pixel typeface.

The camera follows quickjs-oxide. Mountain layers scroll at different rates;
track markings and stage signs move with the world. Cars have animated wheels,
suspension and exhaust; cars outside the camera view retain edge indicators
and minimap markers. When oxide finishes, race positions freeze, confetti and
a finish cue play, and a column shows every engine's recorded relay total.
The original synthesized soundtrack has countdown, stage and motor cues.

The eight cars are quickjs-oxide, QuickJS, JavaScriptCore, V8, Boa, Brimstone,
MuJS and Duktape. All run without JIT: V8 uses Node.js with `--jitless`; JSC
uses `--useJIT=false --useRegExpJIT=false`. Versions and binary/library hashes
are in the campaign record. [Artwork and font notices](assets/README.md)
cover the adapted car construction and rasterized typeface.

## Measurement inputs

The renderer never runs an engine. Measurements use
[`bench.py`](../benchmark/README.md) under the
[measurement rules](../../docs/performance/benchmarking.md): immutable
binaries, fixed repetitions/order, one CPU, serial recording, checked output
and no concurrent builds, tests or encoding.

The frozen campaign contains `plan.json`, `commands.json`, `identities.json`,
`suite/manifest.json`, `aa/results.json`, seven `ab-ENGINE/results.json` files
and seven `paired-ENGINE.json` reports. The A/A uses the same QuickJS binary
under `quickjs`/`control` labels. Each A/B compares QuickJS and one of the other
seven engines. All runs cover the same eight programs with six ABBA pairs,
CPU 2 and Linux counters. Failed or missing samples, changed program bytes,
settings or binary identities, and absent no-JIT flags reject rendering.
A missed per-program gain gate stays a miss in the separate data report.

MuJS and Duktape require ES5 source. The campaign replaces only the generator's
arrow-function print adapter with the same ES5 adapter on every engine; the
benchmark bodies and frozen iteration counts are unchanged. This starts a new
baseline, without mixing earlier timings. Keep the preparation steps and
minimal Duktape host with the raw campaign evidence.

The relay concatenates per-program median process times, including startup,
compilation, execution and exit. Each program occupies exactly one eighth of
the track. Motion within a leg is uniform interpolation, marked on screen as
`中位示意`; the displayed clock is the recorded-time scale, at a fixed 0.4×
playback rate. This is a timing illustration, not a per-iteration trace.
The freeze is at oxide's finish; the time column includes recorded totals for
cars still behind. No geometric-mean scoreboard appears in this cut.

## Render and inspect

Pillow, NumPy and FFmpeg are required. The small pixel glyph atlas and notices
are included; no fonts or packages are installed by the renderer.

```sh
python3 scripts/promo/v8_race.py --campaign target/CAMPAIGN \
  --output target/v8-relay --stills-only
python3 scripts/promo/v8_race.py --campaign target/CAMPAIGN \
  --output target/v8-relay
ffprobe -v error -show_format -show_streams \
  target/v8-relay/quickjs-oxide-v8-v7-relay-only-no-jit-zh.mp4
```

Inspect `contact.png` and `stills/`: the start, each of the eight legs and the
finish. Check label bounds, off-screen indicators, stage order, frozen car
positions and the encoded video through its final frame. Output includes the
MP4, poster, soundtrack, encode log/command, `video-data.json` and a provenance
report. Raw results, media and generated reports stay under `target/` or
outside the repository, out of commits.

The visual reference is the relay in
[Ion_Mio_'s video](https://x.com/Ion_Mio_/status/2119684071299436891).
No frames or audio from it are used in the cut.
