//! Small executable collector decisions verified verbatim by Verus.
//!
//! `scripts/verification/gc.py` adds contracts to these exact function bodies;
//! there is no second implementation of them in the proof directory.

/// Mark a bitmap mask, returning whether at least one bit was newly marked.
#[inline]
pub(crate) fn mark_word(bits: u64, mask: u64) -> (u64, bool) {
    (bits | mask, bits & mask != mask)
}

/// Live records that sweep must release.
#[inline]
pub(crate) fn dead_word(live: u64, marked: u64) -> u64 {
    live & !marked
}

/// 0: no obligation; 1: enqueue the value; 2: wait for the key.
/// The caller establishes map liveness before registering an entry.
#[inline]
pub(crate) fn ephemeron_action(empty: bool, value_live: bool, key_live: bool) -> u8 {
    if empty || value_live {
        0
    } else if key_live {
        1
    } else {
        2
    }
}

/// Adapter operations for one ephemeron closure. `scan` registers newly
/// discovered maps and returns true only with at least one unmarked queued
/// value. `drain` consumes the queue and marks that value (and its closure).
/// `clear` removes deferred entries before a later collection can reuse them.
/// The adapter contracts are specified in verification/gc/driver.rs.in.
pub(crate) trait EphemeronTrace {
    fn scan(&mut self) -> bool;
    fn drain(&mut self);
    fn clear(&mut self);
}

/// Production phase driver; its body is checked verbatim against the adapter
/// contracts. The heap adapter is checked independently under gc-stress.
pub(crate) fn close_ephemerons<T: EphemeronTrace>(trace: &mut T) {
    loop {
        // verification-loop: stable phase boundary
        if !trace.scan() {
            break;
        }
        trace.drain();
    }
    trace.clear();
}
