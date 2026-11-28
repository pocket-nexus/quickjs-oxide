use vstd::std_specs::iter::IteratorSpec;

verus! {
impl StringRepr {
    pub closed spec fn text(&self) -> Seq<u16> {
        match self {
            StringRepr::OneByte(b) => Seq::new(b@.len(), |i:int| b@[i] as u16),
            StringRepr::TwoByte(u) => u@,
        }
    }
}

pub open spec fn hash_prefix(units: Seq<u16>, n: nat) -> u32
    decreases n,
{
    if n == 0 { 0x811c9dc5 } else {
        let prev = (n - 1) as nat;
        vstd::wrapping::u32_specs::wrapping_mul(
            hash_prefix(units, prev) ^ (units[prev as int] as u32), 0x01000193)
    }
}
}
