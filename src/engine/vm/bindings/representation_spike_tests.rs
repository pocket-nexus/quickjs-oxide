//! A4 feasibility evidence, not an alternative executing value representation.
//!
//! Keep every non-Direct variant identical to the real binding. A standalone
//! Vec<u64> comparison would miss the root-bearing variant that sets the stride
//! of SlotStore's actual Vec<Option<FrameBinding>> backing.

use super::{CallableRef, FrameBinding, PrivateNameRef, VarRefRoot};
use crate::engine::value::JsValue;

#[repr(transparent)]
#[expect(dead_code, reason = "Only the counterfactual word layout is measured.")]
struct CandidateWord(u64);

#[expect(
    dead_code,
    reason = "The counterfactual measures real member layouts without constructing owners."
)]
enum CandidateBinding {
    Direct(CandidateWord),
    Private(PrivateNameRef),
    PrivateCallable(CallableRef),
    Uninitialized,
    Captured(VarRefRoot),
}

#[test]
#[cfg(target_pointer_width = "64")]
fn a4_spike_eight_byte_direct_value_does_not_shrink_actual_slot_backing() {
    assert_eq!(size_of::<JsValue>(), 16);
    assert_eq!(size_of::<CandidateWord>(), 8);
    assert_eq!(size_of::<PrivateNameRef>(), 24);
    assert_eq!(size_of::<FrameBinding>(), 32);
    assert_eq!(size_of::<CandidateBinding>(), 32);
    assert_eq!(size_of::<Option<FrameBinding>>(), 32);
    assert_eq!(size_of::<Option<CandidateBinding>>(), 32);
    assert_eq!(align_of::<FrameBinding>(), align_of::<CandidateBinding>());
    // A4 may still shrink separate Vec<JsValue> buffers or RawValue storage.
    // This only rejects attributing an 8B stride to the existing SlotStore.
}
