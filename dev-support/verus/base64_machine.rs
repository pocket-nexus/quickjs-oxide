// One complete or final quantum, including all encoder and decoder modes.
// Arbitrarily many quanta and ill-formed UTF-16 are separate obligations.
#[kani::proof]
#[kani::unwind(66)]
fn encoded_quantum() {
    let source: [u8; 3] = kani::any();
    let len: usize = kani::any();
    kani::assume(len <= 3);
    let url: bool = kani::any();
    let omit: bool = kani::any();
    let mode = match kani::any::<u8>() {
        0 => LastChunk::Loose, 1 => LastChunk::Strict, _ => LastChunk::StopBeforePartial,
    };
    let text = encode_base64(&source[..len], url, omit);
    let units: Vec<u16> = text.as_bytes().iter().map(|&b| u16::from(b)).collect();
    let result = decode_base64(&units, url, mode, usize::MAX);
    let tail = omit && (len == 1 || len == 2);
    if tail && mode != LastChunk::Loose {
        assert!(result.bytes.is_empty() && result.read == 0);
        assert_eq!(result.error, (mode == LastChunk::Strict).then_some("invalid base64 string"));
    } else {
        assert_eq!(result.error, None);
        assert_eq!(result.read, units.len());
        assert_eq!(&result.bytes[..], &source[..len]);
    }
}
