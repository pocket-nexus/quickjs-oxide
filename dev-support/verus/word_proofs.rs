verus! {
// Mathematically this specifies the low word and the carry uniquely.
// The corresponding machine harness checks the identity at width 128.
#[verus::trusted]
#[verifier::external_fn_specification]
pub fn libcore_overflowing_add(x: u64, y: u64) -> (r: (u64, bool))
    ensures r.0 as int + (if r.1 { 0x10000000000000000int } else { 0int }) == x as int + y as int,
{ x.overflowing_add(y) }

#[verus::trusted]
#[verifier::external_fn_specification]
pub fn libcore_carry_bit(value: bool) -> (r: u64)
    ensures r == if value { 1u64 } else { 0u64 },
{ u64::from(value) }

proof fn extra_zeroes(s: Seq<u64>, n: nat)
    requires n >= s.len(),
    ensures digits(s, n) == digits(s, s.len()),
    decreases n - s.len(),
{
    if n > s.len() { extra_zeroes(s, (n - 1) as nat); }
}

proof fn carry_step(x: int, y: int, cin: int, z: int, cout: int, w: int)
    requires x + y + cin == z + 0x10000000000000000int * cout,
    ensures w*x + w*y + w*cin == w*z + (0x10000000000000000int*w)*cout,
{
    assert(w*(x+y+cin) == w*x + w*y + w*cin) by(nonlinear_arith);
    assert(w*(z + 0x10000000000000000int*cout) == w*z + (0x10000000000000000int*w)*cout) by(nonlinear_arith);
}
}
verus! {
#[verus::trusted]
#[verifier::external_fn_specification]
pub fn libcore_overflowing_sub(x: u64, y: u64) -> (r: (u64, bool))
    ensures r.0 as int - (if r.1 { 0x10000000000000000int } else { 0int }) == x as int - y as int,
{ x.overflowing_sub(y) }

proof fn magnitude_bound(s: Seq<u64>, n: nat)
    ensures 0 <= digits(s,n) < place(n), place(n) > 0,
    decreases n,
{
    if n > 0 {
        let prev = (n - 1) as nat;
        magnitude_bound(s, prev);
        let w = place(prev);
        let v = if prev < s.len() { s[prev as int] } else { 0u64 };
        assert(0 <= w*v && w*v <= w*0xffffffffffffffffint) by(nonlinear_arith)
            requires w > 0, 0 <= v <= 0xffffffffffffffffint;
    }
}
}
verus! {
proof fn borrow_step(x:int, y:int, bin:int, z:int, bout:int, w:int)
    requires x-y-bin == z-0x10000000000000000int*bout,
    ensures w*x-w*y-w*bin == w*z-(0x10000000000000000int*w)*bout,
{
    assert(w*(x-y-bin) == w*x-w*y-w*bin) by(nonlinear_arith);
    assert(w*(z-0x10000000000000000int*bout) == w*z-(0x10000000000000000int*w)*bout) by(nonlinear_arith);
}
}

verus! {
#[verus::trusted]
#[verifier::external_fn_specification]
pub fn libcore_unsigned_bytes(x: u16) -> (r: [u8; 2])
    ensures r[0] == x as u8, r[1] == (x >> 8u16) as u8,
{ x.to_le_bytes() }

#[verus::trusted]
#[verifier::external_fn_specification]
pub fn libcore_signed_bytes(x: i16) -> (r: [u8; 2])
    ensures r[0] == x as u8, r[1] == ((x as u16) >> 8u16) as u8,
{ x.to_le_bytes() }
}
