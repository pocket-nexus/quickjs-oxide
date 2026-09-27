# C1: decoded fallthrough for primitive numeric completion

This receipt starts from PR #53 at
`10262309c580c43ba984b4f371acdc081147b4f0`. The change carries the
published instruction's decoded next boundary in `VmAction::Numeric`, through
the ready driver, into same-frame primitive arithmetic completion. The frame
still commits `resume_pc` at the existing completion point. `fault_pc` remains
the attribution site, and decline keeps the action and its position for the
existing fallback protocol.

The covered path is primitive arithmetic after the inline Number path declines,
including string, Boolean, null and BigInt arithmetic. `NumericStep`
comparison completion, object conversion after decline, properties, calls,
delayed replies and suspended frames still use their existing position logic.
The diagnostic event `numeric_legacy_fallthrough_recovery_decode` counts one
of those remaining numeric recovery sites; it does not count ordinary opcode
decoding. No interpreter entry validation was removed.

## Correctness contract

- The carried boundary comes from the originating published instruction, not
  `fault_pc + 1` or a selected branch destination.
- Knowing fallthrough does not commit execution. A decline leaves fault and
  resume positions unchanged; a materialization retry executes the current
  operation again.
- Primitive numeric completion retains the existing output order and commits
  the resume position only after the output pushes succeed. A partial output
  failure leaves the original positions intact.
- The existing frame identity, slot ownership and pending-operation protocols
  remain authoritative. The position wrapper is not an authentication token.

The focused tests inspect the emitted `Sub` and `CompareBranchStack`
instructions, compare carried boundaries with the reference decoder (including
a wide encoding), and cover direct completion, object decline, guest throw
identity and conversion count, partial output failure, and retry. The
profiling-feature tests assert that covered primitive completion occurs and
that its legacy recovery decode count is zero.

## Reproduction and results

The committed `manifest.json` and `workloads/` contain fixed byte hashes and
stdout contracts for primitive, Number-only, object-conversion, comparison,
M1 hit and M1 miss cases. `apps/cli/examples/execute_probe.rs` compiles these
cases once and times only repeated `Context::execute` calls; fixed-runner
timings include CLI startup and compilation. The plain binaries are used for
timing, while profiling binaries establish path coverage.

Results, build identities and machine details will be recorded below after
the fresh parent/candidate builds and validation finish.
