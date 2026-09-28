# Collection-time memory envelope after the cell/shape migration

This receipt compares the current #53 parent (`38e9eb86e2c2db8fb2e607c9f3ff611407941b4b`) with the cell/shape arena candidate plus the typed-ID and single-cell-access follow-ups (`c31a22a87e78d31c6ba9fcb13fd1dc83370414b4`). It is a **host measurement**, not an embedded-target result. The checked-in changes are a probe, runner, temporary instrumentation patches, and observations; the production collector remains unchanged.

The probe creates 32,768 captured cells or distinct shapes, collects while all remain rooted, drops all but 32 roots, runs three explicit collections against the retained slot range, grows a different 4,096-node population, and finally removes all roots. The same JavaScript phases run on both revisions. [Probe source](probe/src/main.rs) and [runner](run.py) specify the sequence and sample validation.

The measurement-only patches place two observations *inside* each collection: before and after finalization. At each point they record the simultaneous capacities of resident arena slot/free-list backing, trial/reachability/worklist/anchor scratch, zero queue, and cleanup vectors. The recorded sum is concurrent at that point. The observations do not include nested payload backing, unrelated runtime allocations, allocator metadata, or a transient reallocation overlap. Process maximum RSS is recorded separately with `/usr/bin/time -l`; it is not added to the arena sum. The two patches are [parent](parent-instrumentation.patch.gz) and [candidate](candidate-instrumentation.patch.gz).

The goal is to answer two questions: whether the candidate's smaller resident slots outweigh its additional collection scratch, and whether a past burst makes later collections of a small rooted graph scan a large retained range. Arena slots are intentionally not shrunk, because doing so without preserving generation history could revive stale handles.

## Results

The balanced run contains 32 successful processes: eight parent and eight candidate samples for each population. The observed capacity values are identical across repetitions. The table gives MiB of **simultaneously reserved arena plus collection-buffer capacity** at the named collection points; the maximum is taken over both instrumented points of all seven explicit collections in each run. Maximum RSS is the median of eight whole-process observations and has a different scope.

| Population and metric | #53 parent | Candidate | Change |
|---|---:|---:|---:|
| Cells: all 32,768 rooted, concurrent capacity | 37.57 MiB | 23.33 MiB | −37.9% |
| Cells: 32 roots after burst, concurrent capacity | 38.19 MiB | 23.71 MiB | −37.9% |
| Cells: highest observed concurrent capacity | 38.57 MiB | 24.09 MiB | −37.5% |
| Cells: median whole-process maximum RSS | 34.96 MiB | 29.27 MiB | −16.3% |
| Shapes: all 32,768 rooted, concurrent capacity | 36.07 MiB | 26.32 MiB | −27.0% |
| Shapes: 32 roots after burst, concurrent capacity | 36.20 MiB | 26.20 MiB | −27.6% |
| Shapes: highest observed concurrent capacity | 36.45 MiB | 26.46 MiB | −27.4% |
| Shapes: median whole-process maximum RSS | 56.33 MiB | 44.38 MiB | −21.2% |

At the burst collection, the candidate's scratch rises by exactly 262,144 bytes in both populations (1.07 to 1.32 MiB), while its arena backing falls by 15.19 MB for cells and 10.48 MB for shapes. Thus scratch does not erase the resident-slot saving in these populations. At the three post-burst collections, both revisions retain roughly 0.57 MiB of scratch capacity even though the collector reports only 375 examined live nodes for cells or 370 for shapes; the retained slot ranges remain about 65,800 entries. This establishes the post-burst *capacity* question, but these samples cannot isolate the cost of scanning those slots from the other collection work.

Host load averages at sample start were roughly 136–155. Collection durations and process wall times are present in the raw records but are **not used for a latency conclusion** under this contention. The slot-capacity sum may exceed process RSS because reserved virtual backing need not be resident. Neither metric is an embedded-target high-water measurement, and the in-collection observations cover only the named buffers at two points rather than every transient allocation.

The [parsed results](data/results.json), [binary/source identities](data/build-identities.json), and [archive of all 96 per-process stdout, stderr, and `/usr/bin/time -l` records](data/raw-samples.tar.gz) preserve every sample. `results.json` contains each archive member's SHA-256 and relative name. Both engines reported the same live-node and cleanup counts at matching phases; the [runner](run.py) now checks that parity automatically and rejects missing or extra explicit phases and malformed capacity observations. The recorded samples are unchanged.

## Reproduction

Use clean detached worktrees at the two commits named above. Copy this receipt's `probe/` directory into the corresponding path under each checkout, decompress and apply only its matching temporary instrumentation patch (`gzip -dc PATH.patch.gz | git apply -`), and build each probe with Rust 1.96.0 on AArch64 macOS:

```sh
RUSTUP_TOOLCHAIN=1.96.0 cargo build --locked --release \
  --manifest-path docs/performance/receipts/arena-gc-envelope-2026-09-28/probe/Cargo.toml \
  --target-dir /tmp/oxide-gc-envelope-PARENT-OR-CANDIDATE-target
```

The nested probe manifest fixes fat LTO and one codegen unit. Run the two resulting binaries on a quiet host with the paired runner:

```sh
python3 docs/performance/receipts/arena-gc-envelope-2026-09-28/run.py \
  --parent /tmp/oxide-gc-envelope-parent-target/release/arena-gc-envelope-probe \
  --candidate /tmp/oxide-gc-envelope-candidate-target/release/arena-gc-envelope-probe \
  --repeats 4 --output /tmp/oxide-gc-envelope-observations
```

The runner checks every explicit phase and both in-collection observations, runs a balanced ABBA/BAAB schedule, and archives stdout, stderr, and whole-process counters for each sample. A quiet host would be needed to study collection latency. To repeat on an embedded target, port the probe and replace macOS process counters with the device's allocator or OS high-water facility while retaining the same phase markers and simultaneous in-collection accounting.
