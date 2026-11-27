// These universal identities discharge the three libcore contracts imported
// by the optional arbitrary-length BigInt magnitude proof.
#[kani::proof]
fn word_arithmetic_contracts() {
    let x: u64 = kani::any();
    let y: u64 = kani::any();
    let bit: bool = kani::any();
    let radix = 1u128 << 64;
    let (sum, carry) = x.overflowing_add(y);
    assert_eq!(u128::from(sum) + if carry { radix } else { 0 },
               u128::from(x) + u128::from(y));
    let (diff, borrow) = x.overflowing_sub(y);
    assert_eq!(u128::from(diff) + u128::from(y),
               u128::from(x) + if borrow { radix } else { 0 });
    assert_eq!(u64::from(bit), if bit { 1 } else { 0 });
}

#[kani::proof]
fn instruction_byte_contracts() {
    let x: u16 = kani::any();
    let expected = [(x % 256) as u8, (x / 256) as u8];
    assert_eq!(x.to_le_bytes(), expected);
    assert_eq!((x as i16).to_le_bytes(), expected);
}
