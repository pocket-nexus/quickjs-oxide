//! Frame-binding representation evidence for T2 and the A4 joint design.
//!
//! Only the first test measures the real `FrameBinding`; every counterfactual
//! uses test-local mirror types so a future A4 change cannot silently rewrite
//! this evidence. The mirrors keep the same member types as the representation
//! they model: a standalone word-size comparison would miss the root-bearing
//! variants that set the stride of `SlotStore`'s actual `Vec<Option<_>>`.

use super::FrameBinding;
use crate::engine::atom::AtomIdx;
use crate::engine::heap::roots::VarRefRoot;
use crate::engine::heap::{ObjectId, VarRefId};
use crate::engine::object::{CallableRef, PrivateNameRef};
use crate::engine::value::JsValue;

/// Pre-T2 layout: rooted wrappers own their edges and have `Drop`.
#[expect(
    dead_code,
    reason = "The counterfactual measures real member layouts without constructing owners."
)]
enum PreHandleBinding {
    Direct(JsValue),
    Private(PrivateNameRef),
    PrivateCallable(CallableRef),
    Uninitialized,
    Captured(VarRefRoot),
}

/// A4's direct value candidate: all 64 payload bits before any tag is charged.
#[repr(transparent)]
struct CandidateWord(u64);

/// Post-A4 counterfactual: 8-byte direct value with today's handle variants.
#[expect(
    dead_code,
    reason = "The counterfactual measures mirror layouts without constructing owners."
)]
enum A4HandleBinding {
    Direct(CandidateWord),
    Private(AtomIdx),
    PrivateCallable(ObjectId),
    Uninitialized,
    Captured(VarRefId),
}

/// T2d counterfactual: binding kind reuses A4's tag space, whole binding u64.
#[repr(transparent)]
struct PackedBinding(u64);

#[test]
#[cfg(target_pointer_width = "64")]
fn t2_slot_backing_is_sixteen_bytes() {
    assert_eq!(size_of::<JsValue>(), 16);
    assert_eq!(size_of::<FrameBinding>(), 16);
    assert_eq!(size_of::<Option<FrameBinding>>(), 16);
    // The tag packs into `JsValue`'s spare discriminant values, so the
    // optional slot needs no extra word and no higher alignment.
    assert_eq!(align_of::<FrameBinding>(), align_of::<JsValue>());
    assert_eq!(align_of::<Option<FrameBinding>>(), align_of::<JsValue>());
}

#[test]
#[cfg(target_pointer_width = "64")]
fn t2_rooted_layout_would_have_kept_a_thirty_two_byte_slot() {
    assert_eq!(size_of::<PrivateNameRef>(), 24);
    assert_eq!(size_of::<CallableRef>(), 16);
    assert_eq!(size_of::<VarRefRoot>(), 16);
    assert_eq!(size_of::<PreHandleBinding>(), 32);
    assert_eq!(size_of::<Option<PreHandleBinding>>(), 32);
}

#[test]
#[cfg(target_pointer_width = "64")]
fn a4_eight_byte_direct_value_alone_does_not_shrink_the_slot() {
    assert_eq!(size_of::<CandidateWord>(), 8);
    assert_eq!(size_of::<AtomIdx>(), 4);
    assert_eq!(size_of::<ObjectId>(), 8);
    assert_eq!(size_of::<VarRefId>(), 8);
    // The 8-byte handles still force an 8-byte payload plus a tag word.
    assert_eq!(size_of::<A4HandleBinding>(), 16);
    assert_eq!(size_of::<Option<A4HandleBinding>>(), 16);
}

#[test]
#[cfg(target_pointer_width = "64")]
fn a4_joint_design_can_pack_kind_and_direct_value_into_one_word() {
    // Optimistic A4 tag budget: nine direct value kinds and five binding kinds
    // must share one tag byte before any payload bits are reserved.
    const A4_VALUE_KIND_BITS: u32 = 4;
    const BINDING_KIND_BITS: u32 = 3;
    const _: () = assert!(A4_VALUE_KIND_BITS + BINDING_KIND_BITS <= u8::BITS);
    assert_eq!(size_of::<PackedBinding>(), 8);
    // A full-range u64 has no niche, so `Option` costs a tag word. An 8-byte
    // slot stride needs a reserved all-zero tag for the empty slot instead.
    assert_eq!(size_of::<Option<PackedBinding>>(), 16);
}
