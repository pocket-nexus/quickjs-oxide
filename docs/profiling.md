# Profiling and diagnostics

The optional `profiling` feature adds compile, VM, allocation, memory and lifecycle diagnostics. The CLI writes diagnostic reports to stderr or to a new JSONL file. [Measurement guidance](performance/measurement.md) describes how profiling data accompanies ordinary release timing.

## Run

```sh
cargo build --locked --release -p quickjs-oxide-cli --features profiling
./target/release/qjs -d workload.js
./target/release/qjs -T workload.js
./target/release/qjs -q -d
./target/release/qjs -dT --profile-json   --profile-output /tmp/oxide-profile.jsonl workload.js
```

`-d/--dump` collects a memory snapshot and compile/VM costs. `-T/--trace` collects arena backing-storage allocation events. `-q -d` samples Runtime and Context lifecycle phases; `--profile-iterations N` controls its sample count. `--profile-events N` controls the trace buffer. Reports preserve program stdout.

JSONL output uses `oxide-memory-v1`, `oxide-allocation-trace-v1`, `oxide-compile-vm-cost-v2` and `oxide-lifecycle-v1` records according to the selected options. Each record contains its scope and accounting fields. A snapshot captures the runtime state at its labelled phase; trace serialization follows Runtime teardown.

## Current data

| Area | Reported values |
| --- | --- |
| Heap | Shared and typed arena slots, free indices, zero queue, object properties, dense elements and buffer backing |
| Code | Published `ExecCode` words, instruction boundaries, static property keys and entered-function instruction counts |
| Compile and VM | Phase timing, execution diagnostics, operation visits, property selection, store completion and frame transitions |
| Allocation trace | Arena Vec capacity changes with stable storage identity and dropped-event count |
| Lifecycle | Runtime/Context construction and teardown samples |

Execution diagnostics identify a function by runtime, bytecode and execution-word PC. They report action-specific handoffs, local completion, selected property outcomes, owner-release categories, frame materialization, repeated PC publication and frame re-entry. Numeric-region diagnostics record selected operations, attempts and guard outcomes. These are logical event counts; ordinary release builds supply timing and hardware counters.

`Runtime::memory_snapshot()` exposes the memory view to Rust embedders. `Runtime::new_with_allocation_trace(host, max_events)` returns a runtime and an allocation-trace handle. The handle supplies its final snapshot after the Context, Value and Runtime owners have been dropped.
