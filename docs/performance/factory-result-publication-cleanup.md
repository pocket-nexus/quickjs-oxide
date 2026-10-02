# Cleanup after a failed factory result publication

Base: `eaf23ba800cd8dd5d2a1005431a5b42e5139c5c6`.
Branch: `fix/vm-factory-result-publication-cleanup`.
Status: the five targeted tests passed. This is an independent correctness
repair, with no performance improvement claim.

The original CreateArray, CreateObject and CreateVariable leaves leaked an owner
when result publication failed. Each factory returns a
rooted ObjectRef; the leaf obtains a second checked raw owner before pushing
the raw JsValue to the operand slot. On push failure the ObjectRef drops, but
the uncommitted raw JsValue has no Drop and its retained edge remains.

This repair shares only that local publication protocol. Success keeps checked
retain, raw slot push and the original ObjectRef Drop/drain point. Checked
retain failure drops the original wrapper exactly as before. On push failure,
the wrapper first drops at its original boundary, then the uncommitted retained
edge is surrendered through the existing void release-or-defer operation. The
original push error is returned. Shared borrows retain the existing deferred
release queue; zero cleanup and factory GC requests keep their original service.

There is no ownership move, idle-state guard, changed successful retain count,
new event counter, callback, scheduling change or generalized ownership trait.
ToObject and other result publishers are outside this isolated change.

Five test sources cover the three factory kinds' successful and rejected
publication, both deferred releases under a shared borrow, original pending-zero
cleanup and checked-retain failure under a mutable borrow. Validation must show
the full-generation Object identity is retired on rejection and remains owned
by the slot on success.

## Validation

Rust 1.88.0 formatted only `src/engine/vm/environment_driver.rs`. The following
targeted command exited successfully, with 5 passed and 0 failed tests:

```sh
CARGO_TARGET_DIR=/home/eric/.cache/oxide-factory-publication-correctness-target cargo +1.88.0 test --lib --features profiling,test262-host --jobs 2 factory_publication_cleanup_tests -- --nocapture
```

Log: `/home/eric/.cache/oxide-factory-publication-correctness-target.log`.
The target process was awaited. This validation ran no full library suite,
release build, benchmark or profiling workload.
