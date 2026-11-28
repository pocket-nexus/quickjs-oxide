// Included in the production heap::atom module during verification.
verus! {
impl View for PropertyKey {
    type V = u32;
    closed spec fn view(&self) -> u32 { self.0 }
}

impl PropertyKey {
    pub closed spec fn encoded(bits: u32) -> PropertyKey { PropertyKey(bits) }
}

pub closed spec fn key_kind_agrees(bits: u32, kind: KeyKind) -> bool {
    match kind {
        KeyKind::Index(i) => bits >= 0x8000_0000u32 && i == bits - 0x8000_0000u32,
        KeyKind::Symbol(id) => 0x4000_0000u32 <= bits < 0x8000_0000u32 && id@ == bits - 0x4000_0000u32,
        KeyKind::String(id) => bits < 0x4000_0000u32 && id@ == bits,
    }
}

pub closed spec fn key_value(bits: u32) -> Value {
    if bits >= 0x8000_0000u32 {
        Value::encoded(0xfff9_0000_0000_0000u64 | ((bits - 0x8000_0000u32) as u64))
    } else if bits >= 0x4000_0000u32 {
        Value::encoded(0xfffd_0000_0000_0000u64 | ((bits - 0x4000_0000u32) as u64))
    } else {
        Value::encoded(0xfffc_0000_0000_0000u64 | (bits as u64))
    }
}

proof fn key_tags()
    ensures INDEX_TAG == 0x8000_0000u32, SYMBOL_TAG == 0x4000_0000u32,
{
    assert(INDEX_TAG == 0x8000_0000u32) by(compute);
    assert(SYMBOL_TAG == 0x4000_0000u32) by(compute);
}

proof fn key_partition(bits: u32)
    ensures
        (bits & INDEX_TAG != 0) == (bits >= 0x8000_0000u32),
        bits < 0x8000_0000u32 ==> ((bits & SYMBOL_TAG != 0) == (bits >= 0x4000_0000u32)),
        bits >= 0x8000_0000u32 ==> bits & !INDEX_TAG == bits - 0x8000_0000u32,
        0x4000_0000u32 <= bits < 0x8000_0000u32 ==> bits & !SYMBOL_TAG == bits - 0x4000_0000u32,
{
    key_tags();
    assert((bits & 0x8000_0000u32 != 0) == (bits >= 0x8000_0000u32)) by(bit_vector);
    assert(bits < 0x8000_0000u32 ==> ((bits & 0x4000_0000u32 != 0) == (bits >= 0x4000_0000u32))) by(bit_vector);
    assert(bits >= 0x8000_0000u32 ==> bits & !0x8000_0000u32 == sub(bits, 0x8000_0000u32)) by(bit_vector);
    assert(0x4000_0000u32 <= bits < 0x8000_0000u32 ==> bits & !0x4000_0000u32 == sub(bits, 0x4000_0000u32)) by(bit_vector);
}
}

macro_rules! closed_const {
    ($(#[$doc:meta])* pub const $name:ident: $ty:ty = $initializer:expr;) => {
        verus! { $(#[$doc])* pub closed const $name: $ty = $initializer; }
    };
}
