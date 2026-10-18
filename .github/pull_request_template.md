## Summary

<!-- What changes and why. -->

## Gates

- [ ] `cargo clippy --workspace --all-targets --all-features -- -D warnings` and `cargo fmt --check`
- [ ] Tests for the changed areas pass

### The core (`core/`, `src/engine/compiler/register`, `src/engine/api/core`)

Only for a pull request that changes code `oxide-core` runs
(the gates in docs/performance/principles.md):

- [ ] `cargo test -p oxide-core --features gc-stress`, the two-core corpus under
      `gc-stress`, and `scripts/checks/check-gc-stress.py`
- [ ] **Loop size:** `nm --size-sort` of `qjs`, symbol
      `oxide_core::vm::interp::run_at`: ____ bytes (gate: 16 KB)
- [ ] **Instruction gate:** `instructions:u` of the eight V8 v7 programs at the
      full fixed work, pinned, against the base branch's `qjs`;
      a program above +2% is explained by a profile or fixed

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

For a pull request that changes the engine or the runner:
`test-test262.sh --focused`, and `--full` when the change can move results.
