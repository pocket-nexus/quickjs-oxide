# Buffers and views

This module implements ArrayBuffer language behavior and the TypedArray and
DataView views built on buffers: construction, length and index conversion,
numeric reads and writes, resize, detach and transfer. Views share the
underlying storage; bytes are not represented as indexed properties of
ordinary objects.

heap owns the raw bytes and shared storage; the builtin layer decides brand
checks, conversion timing, bounds checks and error order. SharedArrayBuffer
and Atomics are implemented in sibling builtin modules that use the same
storage capabilities while keeping the shared-memory rules.

Object argument conversion can run JS and change the buffer or view. Each
operation revalidates metadata and byte access as its semantics require; a
token from a callback-free access does not survive a conversion callback.
TypedArray Number/BigInt element semantics and DataView access stay distinct.

Methods are grouped by algorithm (copy, sort, iteration and so on); those
small directories rely on source-level contracts. See the
[builtins overview](../README.md) for the enclosing responsibilities.
