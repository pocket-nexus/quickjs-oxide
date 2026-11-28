verus! {
// RFC 4648 groups six input bits per symbol, most significant first.
spec fn decoded_group(s: Seq<u32>) -> Seq<u8>
    recommends 2 <= s.len() <= 4,
{
    let first = ((s[0] << 2) | (s[1] >> 4)) as u8;
    if s.len() == 2 { seq![first] } else {
        let second = ((s[1] << 4) | (s[2] >> 2)) as u8;
        if s.len() == 3 { seq![first, second] } else {
            seq![first, second, ((s[2] << 6) | s[3]) as u8]
        }
    }
}

spec fn zero_pad_bits(s: Seq<u32>) -> bool {
    if s.len() == 2 { (s[1] & 15u32) == 0 }
    else if s.len() == 3 { (s[2] & 3u32) == 0 }
    else { true }
}

proof fn unpack(a: u32, b: u32, c: u32, d: u32)
    requires a < 64, b < 64, c < 64, d < 64,
    ensures ({
        let x = (a << 18) | (b << 12) | (c << 6) | d;
        &&& (x >> 16) as u8 == ((a << 2) | (b >> 4)) as u8
        &&& (x >> 8) as u8 == ((b << 4) | (c >> 2)) as u8
        &&& x as u8 == ((c << 6) | d) as u8
        &&& c == 0 && d == 0 ==> (((x >> 8) as u8 == 0) == ((b & 15) == 0))
        &&& d == 0 ==> ((x as u8 == 0) == ((c & 3) == 0))
    })
{
    assert({
        let x = (a << 18) | (b << 12) | (c << 6) | d;
        &&& (x >> 16) as u8 == ((a << 2) | (b >> 4)) as u8
        &&& (x >> 8) as u8 == ((b << 4) | (c >> 2)) as u8
        &&& x as u8 == ((c << 6) | d) as u8
        &&& c == 0 && d == 0 ==> (((x >> 8) as u8 == 0) == ((b & 15) == 0))
        &&& d == 0 ==> ((x as u8 == 0) == ((c & 3) == 0))
    }) by(bit_vector) requires a < 64, b < 64, c < 64, d < 64;
}
}
