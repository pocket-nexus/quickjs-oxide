# Performance race video

`v8_race.py` renders a 76-second, 1080p, Chinese film from an existing V8 v7
measurement campaign. It draws original pixel cars and scenery, synthesizes
an original chiptune soundtrack, and encodes H.264/AAC with FFmpeg. It uses
the installed Pillow, NumPy, Fontconfig, Noto Sans CJK and DejaVu Sans Mono;
it downloads no assets and installs no packages.

The eight cars are quickjs-oxide, QuickJS, JavaScriptCore, V8, Boa, Brimstone,
MuJS and Duktape. **All run without JIT**: V8 uses the Node.js shell with
`--jitless`; JSC uses `--useJIT=false --useRegExpJIT=false`. Pinned versions
and executable/library hashes belong to the campaign record.

## Measure first

All measurements go through
[`scripts/benchmark/bench.py`](../benchmark/README.md); the renderer never
runs an engine. Follow [the measurement rules](../../docs/performance/benchmarking.md):
finish builds first, use immutable binaries, pin one CPU, measure serially,
declare repetitions and order before recording, and keep failed samples.
Do not render or encode video during measurement.

The input layout is:

```text
campaign/
  plan.json                 # date, no-JIT mode, fixed settings and source identity
  commands.json             # commands as run and their exit codes
  identities.json           # binary/library hashes, versions and wrapper flags
  suite/manifest.json       # eight programs, fixed work, hashes and checked stdout
  aa/results.json           # quickjs/control: the same QuickJS binary
  ab-oxide/results.json     # quickjs/oxide
  ab-jsc/results.json       # quickjs/jsc
  ab-v8/results.json        # quickjs/v8
  ab-boa/results.json       # quickjs/boa
  ab-brimstone/results.json # quickjs/brimstone
  ab-mujs/results.json      # quickjs/mujs
  ab-duktape/results.json   # quickjs/duktape
  paired-<engine>.json      # bench.py paired with the matched A/A and net-gain gate
```

Each formal run uses six pairs in ABBA order on CPU 2, with Linux counters;
all eight programs must succeed. The A/A precedes every A/B campaign. A missed
gain gate is preserved and reported; it does not become a passing gate in
the video report. The renderer rejects missing/failed samples, mismatched
program hashes, changed settings or missing explicit no-JIT flags.

MuJS and Duktape need ES5 source. The recording replaces only the suite
generator's arrow-function print adapter with an ES5 print adapter, identically
for every engine. This starts a new baseline: original benchmark bodies and
fixed work counts stay unchanged, and older timings are not mixed into it.
Keep the adapter, minimal Duktape host and exact preparation steps with the
campaign evidence.

## Render and inspect

```sh
python3 scripts/promo/v8_race.py --campaign target/CAMPAIGN \
  --output target/v8-race-video --stills-only
python3 scripts/promo/v8_race.py --campaign target/CAMPAIGN \
  --output target/v8-race-video
ffprobe -v error -show_format -show_streams \
  target/v8-race-video/quickjs-oxide-v8-v7-no-jit-zh.mp4
```

Inspect `contact.png` and the full-resolution `stills/` before encoding. The
output includes the MP4, poster, original soundtrack, encode log/command,
derived `video-data.json` and a report linking each result to its input hash.
Check the encoded video for the full duration, audio, readable text and the
final frame. All media, raw results and reports stay under `target/` or
outside the repository, out of commits.

The relay concatenates each program's median process time, gives every
program an eighth of the track, and uses explicitly labelled uniform motion
inside each leg. It freezes when oxide finishes. It is an illustration of
measured times, not a live race or a recorded per-iteration trace.

The separate scoreboard shows the reciprocal of the equal-weight geometric
mean of each program's median-time ratio to its paired QuickJS run. Bars use
one linear scale. Relay finish times and this relative-speed metric are
different quantities; neither is the original suite's adaptive score. Only
the recorded versions, configurations and machine are compared.

The visual reference is the racing section of
[Ion_Mio_'s video](https://x.com/Ion_Mio_/status/2119684071299436891).
The renderer includes no frames, music, sprites or code from that video.
