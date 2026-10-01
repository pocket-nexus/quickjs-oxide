//! Layout contract for the frame slots used by the production VM.
use super::FrameBinding;
use crate::engine::value::JsValue;

#[test]
#[cfg(target_pointer_width = "64")]
fn optional_frame_binding_keeps_the_direct_value_stride() {
    assert_eq!(size_of::<JsValue>(), 16);
    assert_eq!(size_of::<FrameBinding>(), 16);
    assert_eq!(size_of::<Option<FrameBinding>>(), 16);
    assert_eq!(align_of::<FrameBinding>(), align_of::<JsValue>());
    assert_eq!(align_of::<Option<FrameBinding>>(), align_of::<JsValue>());
}
