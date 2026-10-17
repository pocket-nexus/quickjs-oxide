## Summary

<!-- What changes and why. -->

## Gates

- [ ] `cargo clippy --workspace --all-targets --all-features -- -D warnings` and `cargo fmt --check`
- [ ] Tests for the changed areas pass

### New core (`core/`, `src/engine/compiler/register`, `src/engine/api/new_core*`)

Only for a pull request that changes code the new core runs
(docs/performance/new-core-phase4.md, sections 4.1 and 10):

- [ ] `cargo test -p oxide-core --features gc-stress`, the two-core corpus under
      `new-core-gc-stress`, and `scripts/checks/check-new-core-stress.py`
- [ ] **Loop size:** `nm --size-sort` of `oxide-run`, symbol
      `oxide_core::vm::interp::run_at`: ____ bytes (gate: 16 KB)
- [ ] **Instruction gate:** `instructions:u` of the eight V8 v7 programs at the
      full fixed work, pinned, against phase 3 M6's `oxide-run` (`a25e812d`); a
      program above +2% is explained by a profile or fixed

| Program | Change |
| --- | ---: |
| richards | |
| deltablue | |
| crypto | |
| raytrace | |
| earley-boyer | |
| regexp | |
| splay | |
| navier-stokes | |

### Test262

For a pull request that changes code the current engine runs or the runner:
the current engine's `test-test262.sh --focused` and `--full`; for one that
changes the new core: `--spec dev-support/test262/new-core.conf --focused`.
