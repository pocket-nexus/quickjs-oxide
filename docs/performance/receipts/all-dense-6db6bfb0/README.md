# Full numeric span publication receipt

The test-only probe compiled and published the complete pinned `crypto.js` and
`navier-stokes.js` at engine commit `6db6bfb001916e499ef5ce3303a353c1c42fc396` and
benchmark commit `2034d98fc8c5f8044e186267593f5d5ea5232caf` with Rust 1.94.1.
No benchmark function was run. `receipt.json` records hashes of the inputs,
probe, runner, and output files; `target-counts.tsv` confirms that `am3`,
`lin_solve`, `advect`, and `project` each occurred exactly once.

`all-dense-spans.tsv` records every production `FusionPlan` dense flag at its
actual first and last canonical PCs, with operand indices, opcodes, peak and
delta. The 25 published flags are R0: 5, R1: 4, R2: 1, R3: 12, R4: 1, W0: 1,
and W1: 1. A0–A3, W2, and W3 have no static site in these four functions.

| Function | Published sites |
| --- | --- |
| `am3` | R4 20–24, R2 26–30, R0 51–53 |
| `lin_solve` | W1 42–49; R0 44–46, 114–116, 136–138; R3 141–145, 147–151, 153–157 |
| `advect` | R3 46–50; R0 56–58; R1 136–140, 143–147, 153–157, 160–164 |
| `project` | R3 65–69, 70–74, 76–80, 82–86, 196–200, 201–205, 216–220, 221–225; W0 92–97 |

The W1 and R0 intervals in `lin_solve` overlap intentionally. If W1 commits,
execution skips its interior; if it misses, canonical execution can reach the
R0 flag at PC 44. `dense-sites.tsv` retains the 23 `GetArrayEl`-anchored R0
triad audit: 5 accepted R0, 1 site claimed by a longer candidate at the same
first PC, 13 with a non-direct base in that triad, and 4 with a non-whitelisted
key producer. Those rejection categories describe the R0 triad only; longer
published spans are recorded separately in `all-dense-spans.tsv`.
