# Empty zero-queue retention after explicit collection

This receipt compares #55 at `7599d9890af19739b8024f05c650d3b91fffdbda` with the separate zero-queue candidate at `14218e49b947e1b9173d42955ea39d13eead681d`. The candidate caps an **empty** release queue at 4,096 `RawId` entries after a successful explicit `Runtime::run_gc` and its deferred-root drain. Ordinary releases keep queue capacity for reuse; pending releases are never cleared. On this AArch64 build, `RawId` occupies 12 bytes, so the retained cap is 49,152 bytes (48 KiB), within the selected 64 KiB ceiling.

The [probe](probe/src/main.rs) runs two scenarios for both cells and shapes. The post-burst scenario grows 32,768 nodes, reduces the application roots to 32, collects three times, regrows 4,096 nodes, and tears down. The repeated-burst scenario grows, reduces, and drops a 32,768-node population three times. The [paired runner](run.py) checks phase order and **exact parent/candidate GC parity** for examined nodes, roots, candidate nodes, and all recorded finalization counts. It rejects malformed simultaneous-capacity records and archives every process's stdout, stderr, and macOS process counters.

## Result

Each instrumented comparison has eight parent and eight candidate processes per population/scenario, balanced ABBA/BAAB (64 processes total). The named capacity and growth-event observations were identical within each group. Queue bytes below are measured after the first collection of the 32-root graph; the first collection itself begins with the full buffer, and trimming takes effect only after its final drain.

| Population | Parent retained queue | Candidate retained queue | Saving after trim | Extra queue growth events by teardown, candidate vs parent |
|---|---:|---:|---:|---:|
| Cells, post-burst | 786,432 B | 49,152 B | 737,280 B | 2 |
| Shapes, post-burst | 393,216 B | 49,152 B | 344,064 B | 1 |
| Cells, three repeated bursts | 786,432 B | 49,152 B | 737,280 B | 8 |
| Shapes, three repeated bursts | 393,216 B | 49,152 B | 344,064 B | 6 |

The extra growth events are the cost of giving up capacity before later releases refill the queue. During a repeated burst, the candidate still regrows to the same 786,432-byte cell or 393,216-byte shape queue before trimming again. The policy therefore targets the *small steady state after a burst*, not a lower peak during every subsequent burst.

The measurement-only patches record simultaneous capacity of arena slots/free lists, collection scratch, zero queue, and cleanup vectors at two points inside each explicit collection. The highest observed sum over each complete process was:

| Population/scenario | Parent | Candidate | Change |
|---|---:|---:|---:|
| Cells, post-burst | 25,258,460 B | 24,859,436 B | −1.6% |
| Cells, repeated-burst | 25,904,949 B | 25,167,669 B | −2.8% |
| Shapes, post-burst | 27,743,416 B | 27,602,756 B | −0.5% |
| Shapes, repeated-burst | 28,520,207 B | 28,176,143 B | −1.2% |

The highest-point reduction is smaller than the post-trim queue saving because the first large collection still needs the full queue and subsequent bursts regrow it. These are named-buffer capacity observations, not a complete allocator high-water mark: nested payload allocations, allocator metadata, transient reallocation overlap, and unrelated runtime storage are excluded. Process maximum RSS is recorded separately and is not added to this sum.

Matched **uninstrumented** release probes ran another 64 A/A control and 64 parent/candidate processes. Whole-process median wall ratios for candidate/parent were 0.959 (cells post-burst), 0.951 (cells repeated-burst), 0.931 (shapes post-burst), and 1.030 (shapes repeated-burst). A/A label ratios on identical candidate binaries ranged from 0.960 to 1.057, and individual timings varied more. The host's one-minute load average was roughly 13–15 during these runs. These samples do **not** resolve a collection-latency or throughput benefit or regression. The growth-event counts establish reallocation frequency; they do not measure its isolated time cost. Embedded-target memory and latency remain unmeasured.

The [build identities](data/build-identities.json) record commits, exact binary/source/patch SHA-256 values, host, and Rust 1.96.0. The full [instrumented parsed results](data/instrumented/results.json.gz), [A/A results](data/timing/aa-results.json.gz), [parent/candidate timing results](data/timing/paired-results.json.gz), and adjacent `raw-samples.tar.gz` archives preserve all 192 processes. The JSON is gzip-compressed for repository size; the runner writes plain `results.json` when reproduced. Each result includes SHA-256 values for its archived raw members.

## Reproduction

Use clean detached worktrees at the parent and candidate commits above. Copy this receipt's `probe/` directory into the same path in each worktree. For the instrumented comparison, apply `queue-growth-parent.patch` to the parent and `queue-growth-candidate.patch` to the candidate, then apply `collection-capacity.patch.gz` to each (`gzip -dc PATCH | git apply -`). These patches add observations only; neither is part of the production change. Build each probe with **Rust 1.96.0**, fat LTO, and one codegen unit as fixed by its manifest:

```sh
RUSTUP_TOOLCHAIN=1.96.0 cargo build --locked --release \
  --manifest-path docs/performance/receipts/zero-queue-retention-2026-09-28/probe/Cargo.toml \
  --target-dir /tmp/oxide-zeroq-PARENT-OR-CANDIDATE-target
```

Then run the balanced comparison with the two resulting `arena-gc-envelope-probe` binaries:

```sh
python3 docs/performance/receipts/zero-queue-retention-2026-09-28/run.py \
  --parent /tmp/oxide-zeroq-parent-target/release/arena-gc-envelope-probe \
  --candidate /tmp/oxide-zeroq-candidate-target/release/arena-gc-envelope-probe \
  --repeats 4 --output /tmp/oxide-zeroq-instrumented
```

For timing, rebuild both worktrees with only the copied probe (no temporary instrumentation patches), and add `--timing-only` to the same runner command. Run an A/A control by supplying the identical uninstrumented candidate binary to both engine flags. The recorded host used a `stable` alias that resolved exactly to Rust 1.96.0; the numeric selector above is the reproducible requirement. The runner uses macOS `/usr/bin/time -l`; a device port needs a target-specific process/allocator high-water source.
