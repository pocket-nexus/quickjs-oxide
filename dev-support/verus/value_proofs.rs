// Included in the production value module during verification.
verus! {
impl View for Value {
    type V = u64;
    closed spec fn view(&self) -> u64 { self.0 }
}

impl Value {
    pub closed spec fn encoded(bits: u64) -> Value { Value(bits) }
}

// The representation documented in core/src/value.rs, independent of the lookup table.
pub closed spec fn specified_kind(bits: u64) -> Kind {
    match (bits >> 48u32) as u32 {
        0xfff9 => Kind::Int32,
        0xfffa => match bits & 3u64 {
            0 => Kind::Bool,
            1 => Kind::Undefined,
            2 => Kind::Null,
            _ => Kind::Empty,
        },
        0xfffb => Kind::Object,
        0xfffc => Kind::String,
        0xfffd => Kind::Symbol,
        0xfffe => Kind::BigInt,
        _ => Kind::Number,
    }
}
}

// vstd leaves From<bool> for usize unspecified. Check both possible inputs with
// rustc as well as stating the conversion contract on Value::kind.
const _: () = assert!(usize::from(false) == 0 && usize::from(true) == 1);

// Attach a postcondition to a production associated constant. Verus needs exec
// mode because its initializer calls the checked const fn boxed.
macro_rules! verify_special {
    ($bits:literal; $(#[$doc:meta])* pub const $name:ident: Value = Value(boxed($tag:expr, $payload:expr));) => {
        verus_impl! {
            $(#[$doc])*
            pub exec const $name: Value
                ensures Self::$name@ == $bits
            {
                proof { assert((($tag << 48u32) | (($payload) as u64)) == $bits) by(compute); }
                Value(boxed($tag, $payload))
            }
        }
    };
}

verus! {
// Exercise the executable constructors and accessors together for every payload.
fn payload_round_trips(n: i32, id: u32) {
    let number = Value::int32(n);
    let object = Value::object(ObjectId(id));
    let string = Value::string(StringId(id));
    let symbol = Value::symbol(SymbolId(id));
    let bigint = Value::bigint(BigIntId(id));
    let decoded_number = number.as_int32();
    let decoded_object = object.as_object();
    let decoded_string = string.as_string();
    let decoded_symbol = symbol.as_symbol();
    let decoded_bigint = bigint.as_bigint();
    proof {
        assert((0xfff9u64 << 48u32) == 0xfff9_0000_0000_0000u64) by(compute);
        unpack(0xfff9u64, n as u32);
        assert((0xfffbu64 << 48u32) == 0xfffb_0000_0000_0000u64) by(compute);
        unpack(0xfffbu64, id);
        assert((0xfffcu64 << 48u32) == 0xfffc_0000_0000_0000u64) by(compute);
        unpack(0xfffcu64, id);
        assert((0xfffdu64 << 48u32) == 0xfffd_0000_0000_0000u64) by(compute);
        unpack(0xfffdu64, id);
        assert((0xfffeu64 << 48u32) == 0xfffe_0000_0000_0000u64) by(compute);
        unpack(0xfffeu64, id);
        assert((n as u32) as i32 == n) by(bit_vector);
        assert(decoded_number == Some(n));
        assert(decoded_object == Some(ObjectId(id)));
        assert(decoded_string == Some(StringId(id)));
        assert(decoded_symbol == Some(SymbolId(id)));
        assert(decoded_bigint == Some(BigIntId(id)));
    }
}

proof fn unpack(tag: u64, id: u32)
    requires tag <= 0xffffu64,
    ensures
        ((tag << 48u32) | (id as u64)) >> 48u32 == tag,
        ((tag << 48u32) | (id as u64)) as u32 == id,
{
    assert(tag <= 0xffffu64 ==> (((tag << 48u32) | (id as u64)) >> 48u32) == tag) by(bit_vector);
    assert(((tag << 48u32) | (id as u64)) as u32 == id) by(bit_vector);
}
}
