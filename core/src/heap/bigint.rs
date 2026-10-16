//! BigInt records. Every BigInt is a heap record: an `i64` does not fit the
//! 32-bit payload. Phase 1 defines the record and its tracing only; arithmetic
//! and single-limb small BigInts are phase 4.

use super::arena::Record;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BigIntRecord {
    pub negative: bool,
    /// Magnitude, least significant limb first, no trailing zero limbs.
    pub limbs: Box<[u64]>,
}

impl Record for BigIntRecord {
    fn vacant() -> Self {
        BigIntRecord {
            negative: false,
            limbs: Box::new([]),
        }
    }

    fn extra_bytes(&self) -> usize {
        self.limbs.len() * 8
    }
}
