//! The `Uint8Array` base64 and hex codecs (`FromBase64`, `FromHex` and the
//! encoders of the proposal), over UTF-16 input.

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
const URL_ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

/// `lastChunkHandling`.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum LastChunk {
    Loose,
    Strict,
    StopBeforePartial,
}

/// A decode result: the bytes so far, the input read, and the error that
/// stopped it (a `SyntaxError` message).
pub(crate) struct Decoded {
    pub bytes: Vec<u8>,
    pub read: usize,
    pub error: Option<&'static str>,
}

fn is_whitespace(unit: u16) -> bool {
    matches!(unit, 0x09 | 0x0a | 0x0c | 0x0d | 0x20)
}

fn skip_whitespace(input: &[u16], mut index: usize) -> usize {
    while index < input.len() && is_whitespace(input[index]) {
        index += 1;
    }
    index
}

fn sextet(unit: u16) -> Option<u32> {
    let byte = u8::try_from(unit).ok()?;
    ALPHABET
        .iter()
        .position(|&c| c == byte)
        .map(|index| index as u32)
}

/// `DecodeBase64Chunk`; `None` when strict and the padding bits are set.
fn decode_chunk(chunk: &[u32], strict: bool, out: &mut Vec<u8>) -> bool {
    let mut padded = [0u32; 4];
    padded[..chunk.len()].copy_from_slice(chunk);
    let triple = padded[0] << 18 | padded[1] << 12 | padded[2] << 6 | padded[3];
    let bytes = [(triple >> 16) as u8, (triple >> 8) as u8, triple as u8];
    match chunk.len() {
        2 => {
            if strict && bytes[1] != 0 {
                return false;
            }
            out.push(bytes[0]);
        }
        3 => {
            if strict && bytes[2] != 0 {
                return false;
            }
            out.extend_from_slice(&bytes[..2]);
        }
        _ => out.extend_from_slice(&bytes),
    }
    true
}

/// `FromBase64(string, alphabet, lastChunkHandling, maxLength)`.
pub(crate) fn decode_base64(input: &[u16], url: bool, last: LastChunk, max: usize) -> Decoded {
    let mut bytes = Vec::new();
    let mut read = 0;
    if max == 0 {
        return Decoded {
            bytes,
            read,
            error: None,
        };
    }
    let fail = |bytes: Vec<u8>, read: usize| Decoded {
        bytes,
        read,
        error: Some("invalid base64 string"),
    };
    let mut chunk: Vec<u32> = Vec::with_capacity(4);
    let mut index = 0;
    loop {
        index = skip_whitespace(input, index);
        if index == input.len() {
            if !chunk.is_empty() {
                match last {
                    LastChunk::StopBeforePartial => {
                        return Decoded {
                            bytes,
                            read,
                            error: None,
                        };
                    }
                    LastChunk::Loose => {
                        if chunk.len() == 1 {
                            return fail(bytes, read);
                        }
                        decode_chunk(&chunk, false, &mut bytes);
                    }
                    LastChunk::Strict => return fail(bytes, read),
                }
            }
            return Decoded {
                bytes,
                read: input.len(),
                error: None,
            };
        }
        let mut unit = input[index];
        index += 1;
        if unit == u16::from(b'=') {
            if chunk.len() < 2 {
                return fail(bytes, read);
            }
            index = skip_whitespace(input, index);
            if chunk.len() == 2 {
                if index == input.len() {
                    if last == LastChunk::StopBeforePartial {
                        return Decoded {
                            bytes,
                            read,
                            error: None,
                        };
                    }
                    return fail(bytes, read);
                }
                if input[index] == u16::from(b'=') {
                    index = skip_whitespace(input, index + 1);
                }
            }
            if index < input.len() {
                return fail(bytes, read);
            }
            if !decode_chunk(&chunk, last == LastChunk::Strict, &mut bytes) {
                return fail(bytes, read);
            }
            return Decoded {
                bytes,
                read: input.len(),
                error: None,
            };
        }
        if url {
            if unit == u16::from(b'+') || unit == u16::from(b'/') {
                return fail(bytes, read);
            }
            if unit == u16::from(b'-') {
                unit = u16::from(b'+');
            } else if unit == u16::from(b'_') {
                unit = u16::from(b'/');
            }
        }
        let Some(value) = sextet(unit) else {
            return fail(bytes, read);
        };
        let remaining = max - bytes.len();
        if (remaining == 1 && chunk.len() == 2) || (remaining == 2 && chunk.len() == 3) {
            return Decoded {
                bytes,
                read,
                error: None,
            };
        }
        chunk.push(value);
        if chunk.len() == 4 {
            decode_chunk(&chunk, false, &mut bytes);
            chunk.clear();
            read = index;
            if bytes.len() == max {
                return Decoded {
                    bytes,
                    read,
                    error: None,
                };
            }
        }
    }
}

/// `FromHex(string, maxLength)`.
pub(crate) fn decode_hex(input: &[u16], max: usize) -> Decoded {
    let mut bytes = Vec::new();
    if !input.len().is_multiple_of(2) {
        return Decoded {
            bytes,
            read: 0,
            error: Some("invalid hex string"),
        };
    }
    let digit = |unit: u16| char::from_u32(u32::from(unit)).and_then(|c| c.to_digit(16));
    let mut read = 0;
    while read < input.len() && bytes.len() < max {
        let (Some(high), Some(low)) = (digit(input[read]), digit(input[read + 1])) else {
            return Decoded {
                bytes,
                read,
                error: Some("invalid hex string"),
            };
        };
        read += 2;
        bytes.push((high << 4 | low) as u8);
    }
    Decoded {
        bytes,
        read,
        error: None,
    }
}

/// The base64 text of `bytes`.
pub(crate) fn encode_base64(bytes: &[u8], url: bool, omit_padding: bool) -> String {
    let alphabet = if url { URL_ALPHABET } else { ALPHABET };
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let triple = u32::from(chunk[0]) << 16
            | u32::from(*chunk.get(1).unwrap_or(&0)) << 8
            | u32::from(*chunk.get(2).unwrap_or(&0));
        let symbols = chunk.len() + 1;
        for index in 0..4 {
            if index < symbols {
                out.push(char::from(
                    alphabet[(triple >> (18 - 6 * index) & 63) as usize],
                ));
            } else if !omit_padding {
                out.push('=');
            }
        }
    }
    out
}
