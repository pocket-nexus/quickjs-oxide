// Include a 128th input unit to exercise the first truncated position.
// Compare an arbitrary output position, including the required NUL at 127.
#[kani::proof]
#[kani::unwind(129)]
fn scanner_input_window() {
    let units: [u16; 128] = kani::any();
    let len: usize = kani::any();
    let i: usize = kani::any();
    kani::assume(len <= units.len() && i < 128);
    let bytes = quickjs_parse_bytes(&units[..len]);
    let expected = if i >= 127 || i >= len { 0 }
        else if units[i] <= 255 { units[i] as u8 }
        else if units[i] == 0x2212 { b'-' }
        else { b'x' };
    assert_eq!(bytes[i], expected);
}
