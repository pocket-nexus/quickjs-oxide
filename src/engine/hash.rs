//! Cheap hashers for validated runtime identities and already-hashed bucket keys.
use std::hash::{BuildHasherDefault, Hasher};

#[derive(Default)]
pub(crate) struct FxHasher(u64);
impl Hasher for FxHasher {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        for chunk in bytes.chunks(8) {
            let mut word = [0; 8];
            word[..chunk.len()].copy_from_slice(chunk);
            self.write_u64(u64::from_le_bytes(word));
        }
    }
    fn write_u64(&mut self, value: u64) {
        self.0 = (self.0.rotate_left(5) ^ value).wrapping_mul(0x517cc1b727220a95);
    }
}
pub(crate) type FxBuildHasher = BuildHasherDefault<FxHasher>;

#[derive(Default)]
pub(crate) struct IdentityHasher(u64);
impl Hasher for IdentityHasher {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, _: &[u8]) {
        unreachable!("identity hasher accepts u64 keys only")
    }
    fn write_u64(&mut self, value: u64) {
        self.0 = value;
    }
}
pub(crate) type IdentityBuildHasher = BuildHasherDefault<IdentityHasher>;
