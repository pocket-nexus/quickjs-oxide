// Ten digits suffice for every array index. The length describes the supplied UTF-16 slice.
#[kani::proof]
#[kani::unwind(12)]
fn canonical_index_within_ten_units() {
    let input: [u16; 10] = kani::any();
    let len: usize = kani::any();
    kani::assume(len <= 10);
    let text = &input[..len];
    let mut value = 0u64;
    let mut decimal = len != 0 && (len == 1 || input[0] != 48);
    for &u in text {
        if (48..=57).contains(&u) {
            value = value * 10 + u64::from(u - 48);
        } else {
            decimal = false;
        }
    }
    let expected = (decimal && value <= 4_294_967_294).then_some(value as u32);
    assert_eq!(array_index(len, text.iter().copied()), expected);
}

#[kani::proof]
fn longer_text_is_never_an_index() {
    let len: usize = kani::any();
    kani::assume(len > 10);
    // Reading even one code unit would fail the proof.
    let units = core::iter::from_fn(|| -> Option<u16> { panic!("unexpected index scan") });
    assert!(array_index(len, units).is_none());
}
