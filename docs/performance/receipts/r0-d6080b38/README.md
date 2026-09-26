# R0 published-bytecode coverage receipt

`run_dump.py` captured the complete pinned Crypto and NavierStokes sources at
engine commit `d6080b38687bf014695d9813c681e4d54f13e426` with Rust 1.94.1.
The four target functions each occurred once. This is compile-and-publish
coverage, not runtime frequency or a performance result.

| Function | Published R0 first–last PCs | Other `GetArrayEl` sites |
| --- | --- | ---: |
| `am3` | 20–22, 51–53 | 1 |
| `lin_solve` | 44–46, 114–116, 136–138 | 3 |
| `advect` | 56–58 | 5 |
| `project` | none | 8 |

The production `FusionPlan` published six R0 flags among 23 `GetArrayEl`
instructions. The 17 remaining sites have a non-direct base at the start of
the R0 triad (13) or a key produced by a non-whitelisted instruction (4).
The generated dump and site table are retained outside this PR. R2/R3 and R1
were still unpublished in this R0-only experiment; several rejected sites have
their corresponding longer shapes in the subsequent implementation.
