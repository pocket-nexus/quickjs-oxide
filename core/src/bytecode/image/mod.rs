//! Portable code images of precompiled classic scripts, with a separate
//! verification step before any program code or host callback can run.
//!
//! Images contain compiler output and literal data. Use the same publisher
//! trust policy as for native code: the checksum detects damage and the
//! validator checks publication invariants, neither authenticates a publisher.

mod decode;
mod encode;
mod wire;

use alloc::{boxed::Box, vec::Vec};
use core::fmt;

use super::{CodeUnit, ValidateError};
use crate::{HeapError, StringId, Value};
use wire::{Reader, Writer};

pub use decode::ScriptImage;
pub use encode::encode_script;

const MAGIC: &[u8; 4] = b"OXBC";
const VERSION: u16 = 3;
const HEADER_SIZE: u16 = 24;
const NONE: u32 = u32::MAX;

/// Architecture-independent fingerprint of the interpreter and image schema.
pub const ENGINE_FINGERPRINT: &str = env!("OXIDE_IMAGE_FINGERPRINT");

fn fingerprint() -> u64 {
    u64::from_str_radix(ENGINE_FINGERPRINT, 16).expect("build script emitted a hex fingerprint")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageError {
    Format,
    Version(u16),
    Fingerprint,
    Checksum,
    Malformed,
    InvalidCode(ValidateError),
    UnsupportedScript,
    TooLarge,
}

impl fmt::Display for ImageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Format => f.write_str("invalid Oxide code image header"),
            Self::Version(version) => write!(f, "unsupported Oxide code image version {version}"),
            Self::Fingerprint => f.write_str("Oxide code image engine fingerprint does not match"),
            Self::Checksum => f.write_str("Oxide code image checksum does not match"),
            Self::Malformed => f.write_str("malformed Oxide code image"),
            Self::InvalidCode(error) => error.fmt(f),
            Self::UnsupportedScript => f.write_str("cannot encode this Oxide script"),
            Self::TooLarge => f.write_str("Oxide code image is too large"),
        }
    }
}

impl core::error::Error for ImageError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoadError {
    /// Rejected before instantiation. A host with a compiler may try source.
    Rejected(ImageError),
    /// The accepted image could not be instantiated in this runtime.
    Heap(HeapError),
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rejected(error) => error.fmt(f),
            Self::Heap(error) => error.fmt(f),
        }
    }
}

impl core::error::Error for LoadError {}

struct ImageUnit {
    code: CodeUnit,
    constants: Vec<Constant>,
}

enum Text<'a> {
    Latin1(&'a [u8]),
    Utf16(&'a [u8]),
}

enum Constant {
    Scalar(Value),
    String(u32),
    BigInt {
        negative: bool,
        limbs: Box<[u64]>,
    },
    /// String table indexes, with NONE for invalid escapes in cooked text.
    Template(Box<[(u32, u32)]>),
}

fn text_ref(index: u32, count: usize) -> Result<u32, ImageError> {
    if (index as usize) < count {
        Ok(index)
    } else {
        Err(ImageError::Malformed)
    }
}

fn wire_atom(index: u32) -> StringId {
    StringId(index)
}
