use alloc::vec::Vec;

use super::ImageError;

/// Reading a length never advances past the end of the slice or allocates
/// before establishing a lower bound on its encoded size.
pub(super) struct Reader<'a>(pub(super) &'a [u8]);

impl<'a> Reader<'a> {
    pub(super) fn take(&mut self, len: usize) -> Result<&'a [u8], ImageError> {
        let (head, tail) = self.0.split_at_checked(len).ok_or(ImageError::Malformed)?;
        self.0 = tail;
        Ok(head)
    }

    pub(super) fn u8(&mut self) -> Result<u8, ImageError> {
        Ok(self.take(1)?[0])
    }

    pub(super) fn u16(&mut self) -> Result<u16, ImageError> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }

    pub(super) fn u32(&mut self) -> Result<u32, ImageError> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }

    pub(super) fn u64(&mut self) -> Result<u64, ImageError> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }

    pub(super) fn var_u32(&mut self) -> Result<u32, ImageError> {
        let first = self.u8()?;
        self.var_u32_from(first)
    }

    #[inline]
    pub(super) fn var_u32_from(&mut self, first: u8) -> Result<u32, ImageError> {
        if first < 128 {
            return Ok(u32::from(first));
        }
        let mut value = u32::from(first & 127);
        for shift in [7, 14, 21, 28] {
            let byte = self.u8()?;
            if byte < 128 {
                if byte == 0 || (shift == 28 && byte > 15) {
                    return Err(ImageError::Malformed);
                }
                return Ok(value | (u32::from(byte) << shift));
            }
            value |= u32::from(byte & 127) << shift;
        }
        Err(ImageError::Malformed)
    }

    pub(super) fn signed_delta(&mut self) -> Result<u32, ImageError> {
        let value = self.var_u32()?;
        Ok((value >> 1) ^ 0u32.wrapping_sub(value & 1))
    }

    pub(super) fn count(&mut self, width: usize) -> Result<usize, ImageError> {
        let count = self.var_u32()? as usize;
        if count > self.0.len() / width {
            return Err(ImageError::Malformed);
        }
        Ok(count)
    }

    pub(super) fn blob(&mut self) -> Result<&'a [u8], ImageError> {
        let count = self.count(1)?;
        self.take(count)
    }

    pub(super) fn text(&mut self) -> Result<&'a str, ImageError> {
        core::str::from_utf8(self.blob()?).map_err(|_| ImageError::Malformed)
    }

    pub(super) fn list<T>(
        &mut self,
        width: usize,
        mut item: impl FnMut(&mut Self) -> Result<T, ImageError>,
    ) -> Result<Vec<T>, ImageError> {
        let count = self.count(width)?;
        let mut items = Vec::with_capacity(count);
        for _ in 0..count {
            items.push(item(self)?);
        }
        Ok(items)
    }
}

#[derive(Default)]
pub(super) struct Writer(pub(super) Vec<u8>);

impl Writer {
    pub(super) fn u8(&mut self, value: u8) {
        self.0.push(value);
    }

    pub(super) fn u16(&mut self, value: u16) {
        self.0.extend_from_slice(&value.to_le_bytes());
    }

    pub(super) fn u32(&mut self, value: u32) {
        self.0.extend_from_slice(&value.to_le_bytes());
    }

    pub(super) fn u64(&mut self, value: u64) {
        self.0.extend_from_slice(&value.to_le_bytes());
    }

    pub(super) fn var_u32(&mut self, mut value: u32) {
        while value >= 128 {
            self.u8(value as u8 | 128);
            value >>= 7;
        }
        self.u8(value as u8);
    }

    pub(super) fn signed_delta(&mut self, delta: u32) {
        self.var_u32((delta << 1) ^ ((delta as i32 >> 31) as u32));
    }

    pub(super) fn count(&mut self, count: usize) -> Result<(), ImageError> {
        self.var_u32(u32::try_from(count).map_err(|_| ImageError::TooLarge)?);
        Ok(())
    }

    pub(super) fn blob(&mut self, bytes: &[u8]) -> Result<(), ImageError> {
        self.count(bytes.len())?;
        self.0.extend_from_slice(bytes);
        Ok(())
    }
}

pub(super) fn checksum(bytes: &[u8]) -> u32 {
    const TABLE: [u32; 256] = {
        let mut table = [0; 256];
        let mut index = 0;
        while index < table.len() {
            let mut crc = index as u32;
            let mut bit = 0;
            while bit < 8 {
                crc = (crc >> 1) ^ (0xedb8_8320 & 0u32.wrapping_sub(crc & 1));
                bit += 1;
            }
            table[index] = crc;
            index += 1;
        }
        table
    };
    !bytes.iter().fold(!0, |crc, &byte| {
        (crc >> 8) ^ TABLE[((crc as u8) ^ byte) as usize]
    })
}

/// A running position in the packed source map.
pub(super) struct Positions(crate::bytecode::SourcePosition);

impl Positions {
    pub(super) fn new((line, column): (u32, u32)) -> Self {
        Self(crate::bytecode::SourcePosition {
            pc: 0,
            line,
            column,
        })
    }

    pub(super) fn next(
        &mut self,
        reader: &mut Reader<'_>,
    ) -> Result<crate::bytecode::SourcePosition, ImageError> {
        let head = reader.u8()?;
        let (pc, line, column) = if head < 128 {
            let zigzag = u32::from(head & 15);
            (
                u32::from(head >> 4),
                0,
                (zigzag >> 1) ^ 0u32.wrapping_sub(zigzag & 1),
            )
        } else if head == 128 {
            (
                reader.var_u32()?,
                reader.signed_delta()?,
                reader.signed_delta()?,
            )
        } else {
            return Err(ImageError::Malformed);
        };
        self.0.pc = self.0.pc.checked_add(pc).ok_or(ImageError::Malformed)?;
        self.0.line = self.0.line.wrapping_add(line);
        self.0.column = self.0.column.wrapping_add(column);
        Ok(self.0)
    }

    pub(super) fn write(&mut self, writer: &mut Writer, next: crate::bytecode::SourcePosition) {
        let pc = next.pc - self.0.pc;
        let line = next.line.wrapping_sub(self.0.line);
        let column = next.column.wrapping_sub(self.0.column);
        let zigzag = (column << 1) ^ ((column as i32 >> 31) as u32);
        if pc < 8 && line == 0 && zigzag < 16 {
            writer.u8((pc << 4 | zigzag) as u8);
        } else {
            writer.u8(128);
            writer.var_u32(pc);
            writer.signed_delta(line);
            writer.signed_delta(column);
        }
        self.0 = next;
    }
}

#[cfg(test)]
mod tests {
    use super::{ImageError, Reader, Writer};

    #[test]
    fn variable_integers_preserve_full_width_and_negative_deltas() {
        let values = [0, 1, 127, 128, 0x3fff, 0x4000, 0x8000_0000, u32::MAX];
        let mut writer = Writer::default();
        for value in values {
            writer.var_u32(value);
            writer.signed_delta(value);
        }
        let mut reader = Reader(&writer.0);
        for value in values {
            assert_eq!(reader.var_u32(), Ok(value));
            assert_eq!(reader.signed_delta(), Ok(value));
        }
        assert!(reader.0.is_empty());

        for bytes in [
            &[0x80][..],
            &[0x80, 0],
            &[0x80, 0x80, 0x80, 0x80, 0x10],
            &[0xff, 0xff, 0xff, 0xff, 0xff, 1],
        ] {
            assert_eq!(Reader(bytes).var_u32(), Err(ImageError::Malformed));
        }
    }
}
