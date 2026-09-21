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
    #[inline]
    fn write_u8(&mut self, value: u8) {
        self.write_u64(u64::from(value));
    }
    #[inline]
    fn write_u16(&mut self, value: u16) {
        self.write_u64(u64::from(value.to_le()));
    }
    #[inline]
    fn write_u32(&mut self, value: u32) {
        self.write_u64(u64::from(value.to_le()));
    }
    #[inline]
    fn write_usize(&mut self, value: usize) {
        self.write_u64(value.to_le() as u64);
    }
    #[inline]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_width_hashes_preserve_native_byte_write_mapping() {
        let mut bytes = FxHasher::default();
        let mut words = FxHasher::default();
        for value in [0, 1, 127, 255, 0x12345678, u64::MAX] {
            bytes.write(&(value as u8).to_ne_bytes());
            words.write_u8(value as u8);
            assert_eq!(bytes.finish(), words.finish());
            bytes.write(&(value as u16).to_ne_bytes());
            words.write_u16(value as u16);
            assert_eq!(bytes.finish(), words.finish());
            bytes.write(&(value as u32).to_ne_bytes());
            words.write_u32(value as u32);
            assert_eq!(bytes.finish(), words.finish());
            bytes.write(&(value as usize).to_ne_bytes());
            words.write_usize(value as usize);
            assert_eq!(bytes.finish(), words.finish());
        }
    }
}
