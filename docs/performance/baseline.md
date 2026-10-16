# New core baseline (2026-10-16)

The comparison base of [phase 0](new-core-phase0.md): every later phase of the
new core reports against these numbers, with the same programs, runner and
machine. A change to any identity below starts a new baseline.

## Identity

| Item | Value |
| --- | --- |
| Machine | eric-83am, AMD Ryzen 7 7840HS, Linux 7.2.9, measured processes pinned to CPU 2 |
| Oxide | main `8e16f672` (#124), `build.py --plain-only`, rustc 1.99.0, `qjs` sha256 `3cc9c95d597af1bd…` |
| QuickJS | 2026-06-04 oracle build (upstream Makefile), `qjs` sha256 `3fd16c25ae63a7cb…` |
| Suite programs | `suite_matrix.py build` at this commit; V8 v7 from js-engine-benchmark `2034d98f`, Octane `570ad1cc`, AWFY `74306fec`; manifest sha256 `2f775c66ae9838db…` |
| Probe programs | `docs/performance/probes/fixed/manifest.json`, sha256 `b2ae9f516f6c36de…` |
| Runner | `fixed.py --repeat 5 --cpu 2 --linux-counters --timeout 900`, engine order alternating |

Each cell is the median of 5 runs; all 570 samples produced the exact expected
stdout. Time is whole-process wall time; instructions are user-space
`instructions:u` for the whole process, both including startup and compilation.
The spread (max − min over median) of a cell is 1.9% at the median and 14.1% at
worst for the suite programs; the probes run 20–230 ms, so their spread is
larger (4.1% median, 39.6% worst) and their times are indicative only. Use the
instruction counts for the probes.

```sh
python3 scripts/benchmark/suite_matrix.py build --v8-source … --octane-source … --awfy-source … --output $M
python3 scripts/benchmark/fixed.py --manifest $M/manifest.json \
  --engine quickjs=$QUICKJS --engine oxide=$OXIDE --repeat 5 --cpu 2 --linux-counters --timeout 900 --output $R
python3 scripts/benchmark/suite_matrix.py table $R/results.json --reference quickjs --instructions
```

## V8 v7, Octane 2.0 and AWFY

Oxide is **4.49×** slower than QuickJS and executes **5.19×** its
instructions (geometric means over 37 programs).

| Workload | QuickJS ms | Oxide ms | time ratio | QuickJS G instr | Oxide G instr | instr ratio |
|---|---:|---:|---:|---:|---:|---:|
| v8-v7-richards | 1065 | 4169 | 3.92 | 14.12 | 74.60 | 5.28 |
| v8-v7-deltablue | 1092 | 4765 | 4.36 | 13.32 | 71.89 | 5.40 |
| v8-v7-crypto | 981 | 3448 | 3.52 | 13.86 | 69.15 | 4.99 |
| v8-v7-raytrace | 1083 | 5073 | 4.68 | 15.88 | 65.72 | 4.14 |
| v8-v7-earley-boyer | 1030 | 4457 | 4.33 | 14.23 | 62.68 | 4.40 |
| v8-v7-regexp | 1192 | 1790 | 1.50 | 20.57 | 28.12 | 1.37 |
| v8-v7-splay | 1766 | 5562 | 3.15 | 15.89 | 60.74 | 3.82 |
| v8-v7-navier-stokes | 1050 | 3090 | 2.94 | 14.44 | 65.68 | 4.55 |
| octane-richards | 1062 | 4201 | 3.96 | 14.10 | 74.60 | 5.29 |
| octane-deltablue | 1084 | 4740 | 4.37 | 13.36 | 71.89 | 5.38 |
| octane-crypto | 967 | 3387 | 3.50 | 13.84 | 69.16 | 5.00 |
| octane-raytrace | 1090 | 5021 | 4.61 | 15.92 | 65.73 | 4.13 |
| octane-earley-boyer | 1014 | 4479 | 4.42 | 14.12 | 62.69 | 4.44 |
| octane-regexp | 1252 | 2272 | 1.81 | 21.54 | 35.36 | 1.64 |
| octane-splay | 1774 | 5551 | 3.13 | 15.88 | 60.76 | 3.83 |
| octane-navier-stokes | 1066 | 3090 | 2.90 | 14.44 | 65.74 | 4.55 |
| octane-pdfjs | 1111 | 11796 | 10.62 | 17.68 | 148.80 | 8.42 |
| octane-mandreel | 2392 | 12897 | 5.39 | 33.59 | 229.80 | 6.84 |
| octane-gameboy | 1051 | 7990 | 7.60 | 15.74 | 123.40 | 7.84 |
| octane-code-load | 1014 | 3008 | 2.97 | 11.55 | 27.90 | 2.42 |
| octane-box2d | 1037 | 5157 | 4.98 | 13.00 | 74.14 | 5.70 |
| octane-zlib | 6264 | 30325 | 4.84 | 76.73 | 559.15 | 7.29 |
| octane-typescript | 1650 | 8573 | 5.20 | 15.31 | 95.63 | 6.25 |
| awfy-DeltaBlue | 516 | 2832 | 5.48 | 6.31 | 36.05 | 5.71 |
| awfy-Richards | 3427 | 14344 | 4.19 | 42.58 | 239.46 | 5.62 |
| awfy-Json | 878 | 6528 | 7.44 | 12.15 | 81.64 | 6.72 |
| awfy-CD | 1886 | 9009 | 4.78 | 25.89 | 134.22 | 5.18 |
| awfy-Havlak | 3115 | 18711 | 6.01 | 35.73 | 212.99 | 5.96 |
| awfy-Bounce | 958 | 10133 | 10.58 | 15.15 | 133.83 | 8.83 |
| awfy-List | 943 | 4736 | 5.02 | 12.84 | 93.65 | 7.29 |
| awfy-Mandelbrot | 516 | 2000 | 3.87 | 7.32 | 41.02 | 5.60 |
| awfy-NBody | 745 | 6386 | 8.58 | 11.50 | 94.85 | 8.25 |
| awfy-Permute | 1198 | 5082 | 4.24 | 15.96 | 97.55 | 6.11 |
| awfy-Queens | 778 | 3376 | 4.34 | 10.39 | 62.83 | 6.05 |
| awfy-Sieve | 934 | 7120 | 7.62 | 15.34 | 136.31 | 8.89 |
| awfy-Storage | 1472 | 6589 | 4.48 | 16.41 | 87.85 | 5.35 |
| awfy-Towers | 1198 | 5247 | 4.38 | 14.80 | 94.36 | 6.38 |

- The V8 v7 programs and their Octane counterparts run the same benchmark code
  and agree within 0.1% in instructions and 2% in time, except RegExp, which
  Octane revised.
- RegExp (1.5–1.8×) is the only item near QuickJS: its time is in the regular
  expression engine, which the new core keeps.
- The largest gaps are PdfJS (10.6×), Bounce (10.6×), NBody (8.6×), Sieve
  (7.6×), Gameboy (7.6×) and Json (7.4×). PR #128–#133 target several of them
  in the current core and are not in this baseline.
- The time ratio (4.49× geometric mean) is below the instruction ratio
  (5.19×): Oxide retires more instructions per unit of time than QuickJS, so
  most of the gap is the number of instructions executed. That is what the
  8-byte value, the register VM and the shape ICs of #122 address.

## Fixed probes

Oxide **2.90×** time, **5.02×** instructions (geometric means over 20 probes).

| Workload | QuickJS ms | Oxide ms | time ratio | QuickJS G instr | Oxide G instr | instr ratio |
|---|---:|---:|---:|---:|---:|---:|
| numeric_local_loop | 47 | 143 | 3.04 | 0.50 | 3.03 | 6.06 |
| numeric_coercion_loop | 21 | 231 | 11.21 | 0.15 | 2.62 | 17.47 |
| local_read_with_setup | 54 | 123 | 2.29 | 0.39 | 2.50 | 6.41 |
| local_read | 53 | 123 | 2.30 | 0.39 | 2.50 | 6.41 |
| bigint_32 | 16 | 55 | 3.49 | 0.06 | 0.60 | 10.00 |
| bigint_64 | 21 | 62 | 2.92 | 0.19 | 0.76 | 4.00 |
| bigint_256 | 16 | 35 | 2.22 | 0.08 | 0.32 | 4.00 |
| prop_read | 30 | 59 | 1.95 | 0.40 | 1.09 | 2.73 |
| prop_write | 28 | 97 | 3.51 | 0.33 | 1.92 | 5.82 |
| array_read | 32 | 57 | 1.80 | 0.46 | 1.02 | 2.22 |
| array_write | 30 | 139 | 4.61 | 0.39 | 2.82 | 7.23 |
| call0 | 30 | 115 | 3.78 | 0.31 | 1.79 | 5.77 |
| string_bridge | 19 | 51 | 2.70 | 0.14 | 0.70 | 5.00 |
| type_error | 21 | 46 | 2.20 | 0.17 | 0.45 | 2.65 |
| tdz | 21 | 49 | 2.37 | 0.17 | 0.50 | 2.94 |
| local_move | 55 | 206 | 3.74 | 0.80 | 4.08 | 5.10 |
| argument_move | 54 | 132 | 2.45 | 0.70 | 2.75 | 3.93 |
| number_owner_fallback | 24 | 58 | 2.43 | 0.15 | 1.01 | 6.73 |
| object_move | 20 | 45 | 2.29 | 0.12 | 0.69 | 5.75 |
| empty_loop | 52 | 145 | 2.80 | 0.90 | 2.96 | 3.29 |
