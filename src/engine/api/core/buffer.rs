//! Byte access to `ArrayBuffer`s and the windows of typed arrays.

use super::value::ObjectRef;

/// A typed array's element type.
pub use oxide_core::heap::buffer::ElementKind as TypedArrayKind;

/// A typed array's window on its buffer ([`Js::typed_array_view`](super::Js::typed_array_view)).
#[derive(Clone, Debug, PartialEq)]
pub struct TypedArrayView {
    pub kind: TypedArrayKind,
    pub buffer: ObjectRef,
    /// Where the window starts, in bytes.
    pub byte_offset: usize,
    /// The current length in elements (a length-tracking view follows its
    /// buffer); `None` when the view is out of bounds of its buffer or the
    /// buffer is detached.
    pub length: Option<usize>,
}
