# Execution and performance

The engine compiles each function to register bytecode for its core, `oxide-core`: the front end's stack code is lowered to 8-byte instructions over a 256-register window per frame, validated once when published. One loop, `run_at`, matches on the opcode with `Int32` and double fast paths inline; calls, returns and `new` on bytecode functions stay in the loop, and natives run as Rust functions that read their arguments from the register file and call back into JavaScript re-entrantly. Named property reads and writes, globals and constructions hit 16-byte inline caches guarded by shapes, validity cells and the global object's generation; iteration over unmodified arrays allocates nothing; generators and async functions copy their frame out of the register file when they suspend. Heap records live in typed arenas and a shared value heap, collected by a mark-sweep collector that runs only inside allocating calls.

## Documents

- [Execution](execution.md): frames and register windows, the loop, calls, the inline caches and their hit paths, iteration, suspension and the native ABI.
- [Heap](../heap.md): arenas, the value heap, shapes and validity cells, the collector, its trigger and the heap cap.
- [Execution principles](principles.md): the rooting rule, the `&mut` discipline, the loop-size and instruction gates, and the measurement rule.
- [Measurement method](measurement.md): build identity, fixed workloads, sampling and recorded costs.
- [Diagnostic probes](probes/README.md): the fixed workload matrix, ordinary-write programs, the allocation ledger and the Callgrind runtime probes.
- [Planned work](../planned.md): the heap cap as a policy, reclaiming dynamic code, code images and heap snapshots, 32-bit and `no_std` targets, a generational collector.

## Tools

- [Benchmark tools](../../scripts/benchmark/README.md): build identity, fixed workloads, scaling, compilation, Callgrind, PGO and V8 v7 runs.
- [Diagnostics reference](../profiling.md): `qjs --gc-stats`, `--ic-stats`, `--memory-report` and `--disassemble`.

Performance conclusions are stated per measured source, binary, workload and raw samples, and are drawn from retired instructions; time is reported beside them. A change to an execution path is checked for semantics, the instructions of the fixed workloads and the V8 v7 programs, the loop's size, compile cost and memory.

An optimization first states the target property, scenarios, hard constraints and acceptable trade-offs, then selects a mechanism from a profile or an established resource problem. When the existing interface supports the mechanism, only the implementation changes; when a contract blocks it, types and responsibilities change. The mechanism, the target result and the full cost are verified separately.

Raw samples, logs, profiles, measurement results and experiment archives are kept in `target/` or outside the repository; the repository maintains the current design, measurement method and measurement tools.

